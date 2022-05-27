use inputbot::KeybdKey::{Numpad1Key, Numpad3Key};
use livesplit_core::{Run, Segment, Timer};
use log::*;
use simplelog::{ColorChoice, Config, TermLogger, TerminalMode};
use speedrun_splits::{reset, start_or_split_timer, Speedrun, Splits};
use std::sync::{Arc, RwLock};
use std::thread;

fn main() {
    let _ = TermLogger::init(
        LevelFilter::Trace,
        Config::default(),
        TerminalMode::Mixed,
        ColorChoice::Auto,
    );
    info!("speedrun_splits start");

    let split_names: Vec<String> = vec![
        "Tartarus".to_string(),
        "Asphodel".to_string(),
        "Elysium".to_string(),
        "Styx".to_string(),
        "Hades".to_string(),
    ];
    let splits: Arc<RwLock<Splits>> = Arc::new(RwLock::new(Splits::new(split_names.clone())));
    let splits_ref1: Arc<RwLock<Splits>> = splits.clone();
    let splits_ref2: Arc<RwLock<Splits>> = splits.clone();

    let mut run = Run::new();
    run.set_game_name("Hades");
    run.set_category_name("Clean file");
    for name in split_names {
        run.push_segment(Segment::new(name));
    }

    // Arc allows any thread to point to some variable but it does not allow to
    // mutate it. This is why is wrapping a RwLock
    let t = Arc::new(RwLock::new(Timer::new(run).expect("")));
    let t1 = t.clone();
    let t2 = t.clone();
    let split_key = Numpad1Key;
    info!("split key: {split_key:?}");
    split_key.bind(move || start_or_split_timer(t1.clone(), splits_ref1.clone()));
    let reset_key = Numpad3Key;
    info!("reset key: {reset_key:?}");
    reset_key.bind(move || reset(t2.clone(), splits_ref2.clone()));

    // blocking statement can be handled by spawning its own thread
    thread::spawn(move || {
        // TODO investigate udev for keyboard???
        inputbot::handle_input_events();
    });

    let options = eframe::NativeOptions::default();
    let app = Speedrun::new("Poor man's LiveSplit".to_owned(), t, splits);

    // TODO save state of speedrun

    // also blocking
    eframe::run_native(
        app.get_name().as_str(),
        options,
        Box::new(|_cc| Box::new(app)),
    );
}
