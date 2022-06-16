use clap::{crate_authors, crate_name, crate_version, Arg, Command};
use livesplit_core::hotkey::KeyCode;
use livesplit_core::{Run, Segment, TimeSpan, Timer};
use log::*;
use simplelog::{Config, WriteLogger};
use speedrun_splits::{
    pause, reset, start_or_split_timer, switch_comparison, unpause, Speedrun, Splits,
};
use speedrun_splits::{persistence::*, Keybinding as lKeybinding};
use std::fs;
use std::process::ExitCode;
use std::sync::{Arc, RwLock};

fn main() -> ExitCode {
    let cmd = Command::new(crate_name!())
        .author(crate_authors!())
        .version(crate_version!())
        .about("Splitlive for linux")
        .arg(
            Arg::new("game")
                .requires("category")
                .long("game")
                .long_help("The game name when loading speedrun (\"GAME_CATEGORY\" is the file name searched in data folder)
When used with --force-speedrun-settings-creation, provides the name of the game.")
                .takes_value(true)
                .value_name("GAME"),
        )
        .arg(
            Arg::new("category")
                .requires("game")
                .long("category")
                .long_help("The game category name when loading speedrun (\"GAME_CATEGORY\" is the file name searched in data folder)
When used with --force-speedrun-settings-creation, provides the category name of the game.")
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
                .help("Assign split key (possible values: https://github.com/LiveSplit/livesplit-core/blob/master/crates/livesplit-hotkey/src/key_code.rs)")
                .takes_value(true)
                .value_name("SPLIT KEY"),
            )
        .arg(
            Arg::new("reset-key")
                .short('r')
                .long("reset-key")
                .help("Assign reset key (possible values: https://github.com/LiveSplit/livesplit-core/blob/master/crates/livesplit-hotkey/src/key_code.rs)")
                .takes_value(true)
                .value_name("RESET KEY"),
            )
        .arg(
            Arg::new("pause-key")
                .short('p')
                .long("pause-key")
                .help("Assign pause key (possible values: https://github.com/LiveSplit/livesplit-core/blob/master/crates/livesplit-hotkey/src/key_code.rs)")
                .takes_value(true)
                .value_name("PAUSE KEY"),
            )
        .arg(
            // NOTE: not named resume because short argument conflicts with reset
            Arg::new("unpause-key")
                .short('u')
                .long("unpause-key")
                .help("Assign unpause key (possible values: https://github.com/LiveSplit/livesplit-core/blob/master/crates/livesplit-hotkey/src/key_code.rs)")
                .takes_value(true)
                .value_name("UNPAUSE KEY"),
            )
        .arg(
            Arg::new("comparison-key")
                .short('c')
                .long("comparison-key")
                .help("Assign comparison key to switch between standard comparisons (possible values: https://github.com/LiveSplit/livesplit-core/blob/master/crates/livesplit-hotkey/src/key_code.rs)")
                .takes_value(true)
                .value_name("COMPARISON KEY"),
            )
        .arg(
            Arg::new("icons")
                .short('i')
                .long("icons")
                .help("Give icon filepath for speedrun creation")
                .takes_value(true)
                .multiple_values(true)
                .value_name("ICON FILEPATH"),
            )
        .arg(
            Arg::new("accept-automatically-configuration-creation")
                .long("accept-automatically-configuration-creation")
                .help("Create configuration for speedrun_splits app and skip dialog")
            )
        .arg(
            Arg::new("force-speedrun-settings-creation")
                .long("force-speedrun-settings-creation")
                .help("Avoid behavior where program defaults to finding speedrun by name.")
            )
        .arg(
            Arg::new("make-speedrun-default")
                .long("make-speedrun-default")
                .help("Make created speedrun default")
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
    let icons = m.values_of("icons");
    let accept_speedrun_splits_configuration_creating_dialog =
        m.is_present("accept-automatically-configuration-creation");
    let force_ss_creation = m.is_present("force-speedrun-settings-creation");
    let make_speedrun_default = m.is_present("make-speedrun-default");

    let config = match parse_configuration(accept_speedrun_splits_configuration_creating_dialog) {
        Ok(c) => c,
        Err(e) => {
            error!("{e}");
            // TODO user facing message
            return std::process::ExitCode::FAILURE;
        }
    };
    let keybinding =
        UserKeybinding::new(split_key, reset_key, pause_key, unpause_key, comparison_key);
    let (settings, image_names, is_new) = match load_speedrun_settings(
        &config,
        game,
        category,
        split_names,
        keybinding,
        icons,
        force_ss_creation,
    ) {
        Ok((ss, n, is_new)) => (ss, n, is_new),
        Err(e) => match e {
            SpeedrunSettingsFileError::UserCancel() => {
                info!("{e}");
                return std::process::ExitCode::SUCCESS;
            }
            _ => {
                error!("{e}");
                debug!("{e:?}");
                // TODO user facing message
                return std::process::ExitCode::FAILURE;
            }
        },
    };

    if is_new {
        if let Err(e) =
            update_configuration_with_default_speedrun(config, &settings, make_speedrun_default)
        {
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
    for (i, name) in settings.get_split_names().iter().enumerate() {
        let mut s = Segment::new(name);
        if let Some(names) = image_names.clone() {
            if let Some(path) = names.get(i) {
                let img = vec![];
                let image = match livesplit_core::settings::Image::from_file(path, img) {
                    Ok(i) => i,
                    Err(e) => {
                        error!("{e}");
                        // TODO user facing message
                        return std::process::ExitCode::FAILURE;
                    }
                };
                s.set_icon(image);
            }
        }
        run.push_segment(s);
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

    debug!("{:?}", KeyCode::Numpad1);
    debug!("{:#?}", KeyCode::Numpad1);

    let split_key = settings.get_split_key();
    info!("split key: {split_key:?}");

    let hook = livesplit_core::hotkey::Hook::new().unwrap();
    if let Err(e) = hook.register(split_key, move || {
        start_or_split_timer(t1.clone(), splits_ref1.clone())
    }) {
        error!("{e}");
        // TODO user facing message
        return std::process::ExitCode::FAILURE;
    }

    let reset_key = settings.get_reset_key();
    info!("reset key: {reset_key:?}");
    if let Err(e) = hook.register(reset_key, move || reset(t2.clone(), splits_ref2.clone())) {
        error!("{e}");
        // TODO user facing message
        return std::process::ExitCode::FAILURE;
    }

    let pause_key = settings.get_pause_key();
    info!("pause key: {pause_key:?}");
    if let Err(e) = hook.register(pause_key, move || pause(t3.clone())) {
        error!("{e}");
        // TODO user facing message
        return std::process::ExitCode::FAILURE;
    }

    let unpause_key = settings.get_unpause_key();
    info!("unpause key: {unpause_key:?}");
    if let Err(e) = hook.register(unpause_key, move || unpause(t4.clone())) {
        error!("{e}");
        // TODO user facing message
        return std::process::ExitCode::FAILURE;
    }

    let comparison_key = settings.get_comparison_key();
    info!("comparison key: {comparison_key:?}");
    if let Err(e) = hook.register(comparison_key, move || {
        switch_comparison(t5.clone(), splits_ref3.clone())
    }) {
        error!("{e}");
        // TODO user facing message
        return std::process::ExitCode::FAILURE;
    }

    let options = eframe::NativeOptions::default();
    let keybinding = lKeybinding::new(split_key, reset_key, pause_key, unpause_key, comparison_key);
    let mut app = Speedrun::new(
        "Poor man's LiveSplit".to_owned(),
        t,
        splits,
        keybinding,
        settings,
    );
    if let Err(e) = app.init() {
        error!("{e}");
        // TODO user facing message
        return std::process::ExitCode::FAILURE;
    }

    // also blocking
    eframe::run_native(
        app.get_name().as_str(),
        options,
        Box::new(|_cc| Box::new(app)),
    );
}
