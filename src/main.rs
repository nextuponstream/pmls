use clap::{crate_authors, crate_name, crate_version, Arg, Command};
use livesplit_core::{Run, Segment, TimeSpan, Timer};
use log::*;
use simplelog::{Config, WriteLogger};
use speedrun_splits::{
    parse_key, pause, reset, start_or_split_timer, switch_comparison, unpause, Speedrun, Splits,
};
use speedrun_splits::{persistence::*, Keybinding as lKeybinding};
use std::fs;
use std::process::ExitCode;
use std::sync::{Arc, RwLock};
use std::thread;

fn main() -> ExitCode {
    let cmd = Command::new(crate_name!())
        .author(crate_authors!())
        .version(crate_version!())
        .about("Splitlive for linux")
        .arg(
            Arg::new("game")
                .requires("category")
                .short('g')
                .long("game")
                .help("The game name when loading speedrun (\"{game}_{category}\" is the file name searched in data folder)")
                .takes_value(true)
                .value_name("GAME"),
        )
        .arg(
            Arg::new("category")
                .requires("game")
                .short('c')
                .long("category")
                .help("The game category name when loading speedrun (\"{game}_{category}\" is the file name searched in data folder)")
                .takes_value(true)
                .value_name("CATEGORY"),
        )
        .arg(
            Arg::new("split-names")
                .short('n')
                .long("split-names")
                .help("The split names when creating a speedrun")
                .takes_value(true)
                .value_name("CATEGORY"),
        )
        .arg(
            Arg::new("split-key")
                .short('s')
                .long("split-key")
                .help("Assign split key (possible values: https://github.com/obv-mikhail/InputBot/blob/develop/src/public.rs)")
                .takes_value(true)
                .value_name("SPLIT KEY"),
            )
        .arg(
            Arg::new("reset-key")
                .short('r')
                .long("reset-key")
                .help("Assign reset key (possible values: https://github.com/obv-mikhail/InputBot/blob/develop/src/public.rs)")
                .takes_value(true)
                .value_name("RESET KEY"),
            )
        .arg(
            Arg::new("pause-key")
                .short('p')
                .long("pause-key")
                .help("Assign pause key (possible values: https://github.com/obv-mikhail/InputBot/blob/develop/src/public.rs)")
                .takes_value(true)
                .value_name("PAUSE KEY"),
            )
        .arg(
            // NOTE: not named resume because short argument conflicts with reset
            Arg::new("unpause-key")
                .short('u')
                .long("unpause-key")
                .help("Assign unpause key (possible values: https://github.com/obv-mikhail/InputBot/blob/develop/src/public.rs)")
                .takes_value(true)
                .value_name("UNPAUSE KEY"),
            )
        .arg(
            Arg::new("comparison-key")
                .short('c')
                .long("comparison-key")
                .help("Assign comparison key to switch between standard comparisons (possible values: https://github.com/obv-mikhail/InputBot/blob/develop/src/public.rs)")
                .takes_value(true)
                .value_name("COMPARISON KEY"),
            )
        .after_help(
            "This command requires privilege over one keyboard device. It is \
NOT advised to run this program with sudo. The recommended way (to avoid most \
security issues) is to change the group owner of your external keyboard \
device temporarily to the current $USER.

1. identify the external keyboard device with: `ls -la /dev/input/by-id`
2. change group: `sudo chgrp $USER /dev/input/eventXXX`
3. run the program `/path/to/speedrun_splits`
4. when finished, unplug and plug again the external keyboard to reset the
   group owner (by default, it's \"input\")

When executed as $USER (find the value with `echo $USER`), files will be \
placed under:
* /home/<USER>/.speedrun_splits
* /home/<USER>/.config/.speedrun_splits

Note: if ran with sudo, replace /home/<USER> with /root

Note2: if you are willing to introduce a vulnerability permanently, add $USER \
to \"input\" group (group owner of eventXXX (`ls -la /dev/input/`))
",
        );
    let m = cmd.clone().get_matches();

    // create default data directory
    let default_data_folder = match default_data_folder() {
        Ok(f) => f,
        Err(e) => {
            error!("{e}");
            // TODO user facing message
            return std::process::ExitCode::FAILURE;
        }
    };
    if !std::path::Path::new(default_data_folder.as_str()).exists() {
        fs::create_dir(default_data_folder).unwrap();
    }

    // don't log until --help is parsed
    let default_log_file_path = match default_log_file_path() {
        Ok(f) => f,
        Err(e) => {
            error!("{e}");
            // TODO user facing message
            return std::process::ExitCode::FAILURE;
        }
    };

    let f = match fs::OpenOptions::new()
        .append(true)
        .write(true)
        .create(true)
        .open(default_log_file_path)
    {
        Ok(f) => f,
        Err(e) => {
            println!("{e}");
            // TODO user facing message
            return std::process::ExitCode::FAILURE;
        }
    };
    let _ = WriteLogger::init(LevelFilter::Trace, Config::default(), f);
    info!("speedrun_splits start");
    match cmd.get_version() {
        Some(v) => info!("Version: {v}"),
        None => {
            warn!("Unknown version of application");
        }
    }

    // user arguments to load specific speedrun
    let game = m.value_of("game");
    let category = m.value_of("category");
    let split_names = m.value_of("split-names");
    let split_key = m.value_of("split-key");
    let reset_key = m.value_of("reset-key");
    let pause_key = m.value_of("pause-key");
    let unpause_key = m.value_of("unpause-key");
    let comparison_key = m.value_of("comparison-key");

    let config = match parse_configuration() {
        Ok(c) => c,
        Err(e) => {
            error!("{e}");
            // TODO user facing message
            return std::process::ExitCode::FAILURE;
        }
    };
    let keybinding =
        UserKeybinding::new(split_key, reset_key, pause_key, unpause_key, comparison_key);
    let (settings, is_new) =
        load_speedrun_settings(&config, game, category, split_names, keybinding);
    let settings = match settings {
        Ok(s) => s,
        Err(e) => {
            error!("{e}");
            // TODO user facing message
            return std::process::ExitCode::FAILURE;
        }
    };

    if is_new {
        if let Err(e) = update_configuration_with_default_speedrun(config.clone(), &settings) {
            error!("{e}");
            // TODO user facing message
            return std::process::ExitCode::FAILURE;
        }
    }
    if let Err(e) = save_speedrun_settings_to_file(&settings) {
        error!("{e}");
        // TODO user facing message
        return std::process::ExitCode::FAILURE;
    }

    let splits: Arc<RwLock<Splits>> =
        Arc::new(RwLock::new(Splits::new(settings.get_split_names())));
    let splits_ref1: Arc<RwLock<Splits>> = splits.clone();
    let splits_ref2: Arc<RwLock<Splits>> = splits.clone();
    let splits_ref3: Arc<RwLock<Splits>> = splits.clone();

    let mut run = Run::new();
    run.set_game_name(&settings.get_game_name());
    run.set_category_name(&settings.get_category_name());
    for name in &settings.get_split_names() {
        run.push_segment(Segment::new(name));
    }

    // save run and initialize current comparison
    match parse_run_from_file(&settings) {
        Ok(parsed_run) => {
            run = parsed_run;
        }
        Err(e) => {
            // if file does not exists yet, don't exit yet
            warn!("Could not parse run file: {e}");
        }
    };

    if let Err(e) = save_run_to_file(&run, &settings) {
        error!("{e}");
        // TODO user facing message
        return std::process::ExitCode::FAILURE;
    }

    // Arc allows any thread to point to some variable but it does not allow to
    // mutate it. This is why is wrapping a RwLock
    let t = Arc::new(RwLock::new(Timer::new(run.clone()).expect("")));

    // load current comparison into the UI
    match t.read() {
        Ok(timer) => {
            let mut splits = splits.write().unwrap();
            for (i, s) in run.segments().iter().enumerate() {
                let comparison = s.comparison(timer.current_comparison());
                if let Some(loaded_comparison) = comparison.real_time {
                    splits.update_split(i, TimeSpan::zero(), loaded_comparison);
                }
            }
        }
        Err(e) => {
            error!("{e}");
            // TODO user facing message
            return std::process::ExitCode::FAILURE;
        }
    };

    let t1 = t.clone();
    let t2 = t.clone();
    let t3 = t.clone();
    let t4 = t.clone();
    let t5 = t.clone();

    let split_key = match parse_key(settings.get_split_key()) {
        Ok(k) => k,
        Err(e) => {
            error!("{e}");
            return std::process::ExitCode::FAILURE;
        }
    };
    info!("split key: {split_key:?}");
    split_key.bind(move || start_or_split_timer(t1.clone(), splits_ref1.clone()));

    let reset_key = match parse_key(settings.get_reset_key()) {
        Ok(k) => k,
        Err(e) => {
            error!("{e}");
            return std::process::ExitCode::FAILURE;
        }
    };
    info!("reset key: {reset_key:?}");
    reset_key.bind(move || reset(t2.clone(), splits_ref2.clone()));

    let pause_key = match parse_key(settings.get_pause_key()) {
        Ok(k) => k,
        Err(e) => {
            error!("{e}");
            return std::process::ExitCode::FAILURE;
        }
    };
    info!("pause key: {pause_key:?}");
    pause_key.bind(move || pause(t3.clone()));

    let unpause_key = match parse_key(settings.get_unpause_key()) {
        Ok(k) => k,
        Err(e) => {
            error!("{e}");
            return std::process::ExitCode::FAILURE;
        }
    };
    info!("unpause key: {unpause_key:?}");
    unpause_key.bind(move || unpause(t4.clone()));

    let comparison_key = match parse_key(settings.get_comparison_key()) {
        Ok(k) => k,
        Err(e) => {
            error!("{e}");
            return std::process::ExitCode::FAILURE;
        }
    };
    info!("comparison key: {comparison_key:?}");
    comparison_key.bind(move || switch_comparison(t5.clone(), splits_ref3.clone()));

    // blocking statement can be handled by spawning its own thread
    thread::spawn(move || {
        inputbot::handle_input_events();
    });

    // NOTE: for debug purposes, some keys may be "unregistered"
    // ex: Numpad1Key works but not 2, 4, 6, 7, 8 or FX keys
    //inputbot::KeybdKey::bind_all(|event| {
    //    match inputbot::from_keybd_key(event) {
    //        Some(c) => println!("{c}"),
    //        None => println!("Unregistered Key"),
    //    };
    //});

    let options = eframe::NativeOptions::default();
    let keybinding = lKeybinding::new(split_key, reset_key, pause_key, unpause_key, comparison_key);
    let app = Speedrun::new(
        "Poor man's LiveSplit".to_owned(),
        t,
        splits,
        keybinding,
        settings,
    );

    // also blocking
    eframe::run_native(
        app.get_name().as_str(),
        options,
        Box::new(|_cc| Box::new(app)),
    );
}
