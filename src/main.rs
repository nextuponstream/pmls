use eframe::egui;
use inputbot::KeybdKey::Numpad1Key;
use inputbot::KeybdKey::Numpad3Key;
use livesplit_core::TimerPhase::*;
use livesplit_core::{Run, Segment, Timer};
use std::sync::{Arc, RwLock};
use std::thread;

struct MyApp {
    name: String,
    timer: Arc<RwLock<Timer>>,
    splits: Arc<RwLock<[String; 5]>>,
}

impl MyApp {
    pub fn new(name: String, timer: Arc<RwLock<Timer>>, splits: Arc<RwLock<[String; 5]>>) -> Self {
        Self {
            name,
            timer,
            splits,
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let splits = self.splits.read().unwrap();
        println!("{splits:?}");
        let current_time = {
            let d = self
                .timer
                .read()
                .unwrap()
                .snapshot()
                .current_time()
                .real_time
                .unwrap()
                .to_duration();
            format!(
                "{}h{}m{}.{}",
                d.whole_hours(),
                d.whole_minutes(),
                d.whole_seconds(),
                d.whole_milliseconds()
            )
        };
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading(self.name.clone());
            ui.horizontal(|ui| {
                ui.label("Tartarus: ");
                ui.label(splits[0].to_string());
            });
            ui.horizontal(|ui| {
                ui.label("Asphodel: ");
                ui.label(splits[1].to_string());
            });
            ui.horizontal(|ui| {
                ui.label("Elysium: ");
                ui.label(splits[2].to_string());
            });
            ui.horizontal(|ui| {
                ui.label("Styx: ");
                ui.label(splits[3].to_string());
            });
            ui.horizontal(|ui| {
                ui.label("Hades: ");
                ui.label(splits[4].to_string());
            });
            ui.horizontal(|ui| {
                ui.label("Time: ");
                ui.label(current_time);
            });
        });

        // continously repaint even if out of focus
        ctx.request_repaint();
    }
}

/// Starts the timer with the relevant keybinding and prints a message
fn start_or_split_timer(timer: Arc<RwLock<Timer>>, splits: Arc<RwLock<[String; 5]>>) {
    let mut splits = splits.write().unwrap();
    let message = match timer.read().unwrap().current_phase() {
        // FIXME wrong start message
        Running | Ended => "Start!",
        _ => "Split!",
    };
    println!("{}", message);
    timer.write().unwrap().split_or_start();
    let timer = timer.read().unwrap();
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
    println!("split: {splits:?}");
}

/// Prints current timer and split
fn print_timer(timer: Arc<RwLock<Timer>>) {
    let timer = timer.read().unwrap();
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

fn main() {
    let splits: Arc<RwLock<[String; 5]>> = Default::default();
    let splits_ref1: Arc<RwLock<[String; 5]>> = splits.clone();

    let mut run = Run::new();
    run.set_game_name("Hades");
    run.set_category_name("Clean file");
    run.push_segment(Segment::new("Tartarus"));
    run.push_segment(Segment::new("Asphodel"));
    run.push_segment(Segment::new("Elysium"));
    run.push_segment(Segment::new("Styx"));
    run.push_segment(Segment::new("Hades"));

    // Arc allows any thread to point to some variable but it does not allow to
    // mutate it. This is why is wrapping a RwLock
    let t = Arc::new(RwLock::new(Timer::new(run).expect("")));
    let t1 = t.clone();
    let t2 = t.clone();
    Numpad1Key.bind(move || start_or_split_timer(t1.clone(), splits_ref1.clone()));
    Numpad3Key.bind(move || print_timer(t2.clone()));

    // blocking statement can be handled by spawning it's own thread
    thread::spawn(move || {
        // TODO investigate udev for keyboard???
        inputbot::handle_input_events();
    });

    let options = eframe::NativeOptions::default();
    let app = MyApp::new("Poor man's LiveSplit".to_owned(), t, splits);

    // also blocking
    eframe::run_native(
        app.name.clone().as_str(),
        options,
        Box::new(|_cc| Box::new(app)),
    );
}
