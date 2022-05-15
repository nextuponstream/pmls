use eframe::egui;
use inputbot::KeybdKey::Numpad1Key;
use inputbot::KeybdKey::Numpad3Key;
use lazy_static::lazy_static;
use livesplit_core::TimerPhase::*;
use livesplit_core::{Run, Segment, Timer};
use std::sync::Mutex;
use std::thread;

struct MyApp {
    name: String,
    splits: &'static Mutex<[String; 5]>,
}

impl MyApp {
    pub fn new(name: String, splits: &'static Mutex<[String; 5]>) -> Self {
        Self { name, splits }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        println!("update");
        let splits = self.splits.lock().unwrap();
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading(self.name.clone());
            ui.horizontal(|ui| {
                ui.label("Tartarus: ");
                ui.label(format!("{}", splits[0]));
            });
            ui.horizontal(|ui| {
                ui.label("Asphodel: ");
                ui.label(format!("{}", splits[1]));
            });
            ui.horizontal(|ui| {
                ui.label("Elysium: ");
                ui.label(format!("{}", splits[2]));
            });
            ui.horizontal(|ui| {
                ui.label("Styx: ");
                ui.label(format!("{}", splits[3]));
            });
            ui.horizontal(|ui| {
                ui.label("Hades: ");
                ui.label(format!("{}", splits[4]));
            });
        });
    }
}

/// Starts the timer with the relevant keybinding and prints a message
fn start_or_split_timer(timer: &Mutex<Timer>, splits: &Mutex<[String; 5]>) {
    let mut splits = splits.lock().unwrap();
    let mut timer = timer.lock().unwrap();
    let message = match timer.current_phase() {
        // FIXME wrong start message
        Running | Ended => "Start!",
        _ => "Split!",
    };
    println!("{}", message);
    timer.split_or_start();
    let snapshot = timer.snapshot();
    let segments = snapshot.run().segments();
    for i in 0..5 {
        if let Some(real_time) = segments[i].split_time().real_time {
            let d = real_time.to_duration();
            splits[i] = format!(
                "{}h{}m{}.{}",
                d.whole_hours(),
                d.whole_minutes(),
                d.whole_seconds(),
                d.whole_milliseconds()
            );
        };
    }
    println!("{:?}", segments);
}

/// Prints current timer and split
fn print_timer(timer: &Mutex<Timer>) {
    let timer = timer.lock().unwrap();

    let d = timer
        .snapshot()
        .current_time()
        .real_time
        .unwrap()
        .to_duration();
    let phase = timer.current_split().unwrap().name();
    println!(
        "{}\t:{}h{}m{}.{}",
        phase,
        d.whole_hours(),
        d.whole_minutes(),
        d.whole_seconds(),
        d.whole_milliseconds()
    );
}

lazy_static! {
    static ref TIMER: Mutex<Timer> = {
        let mut run = Run::new();
        run.set_game_name("Hades");
        run.set_category_name("Clean file");
        run.push_segment(Segment::new("Tartarus"));
        run.push_segment(Segment::new("Asphodel"));
        run.push_segment(Segment::new("Elysium"));
        run.push_segment(Segment::new("Styx"));
        run.push_segment(Segment::new("Hades"));
        let t = Mutex::new(Timer::new(run).expect(""));

        t
    };
}

fn main() {
    let splits: &'static Mutex<[String; 5]> = Box::leak(Box::new(Default::default()));
    // can't borrow if timer is not in lazy static or some Boxed things found on Stackoverflow
    Numpad1Key.bind(|| start_or_split_timer(&TIMER, splits));
    Numpad3Key.bind(|| print_timer(&TIMER));

    // blocking statement can be handled by spawning it's own thread
    thread::spawn(move || {
        inputbot::handle_input_events();
    });

    // also blocking
    let options = eframe::NativeOptions::default();
    let app = MyApp::new("Poor man's LiveSplit".to_owned(), splits);
    eframe::run_native("My egui App", options, Box::new(|_cc| Box::new(app)));
}
