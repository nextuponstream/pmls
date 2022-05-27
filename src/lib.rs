use eframe::egui;
use livesplit_core::TimeSpan;
use livesplit_core::Timer;
use livesplit_core::TimerPhase::*;
use std::sync::{Arc, RwLock};

pub struct Speedrun {
    name: String,
    timer: Arc<RwLock<Timer>>,
    splits: Arc<RwLock<Splits>>,
}

impl Speedrun {
    pub fn new(name: String, timer: Arc<RwLock<Timer>>, splits: Arc<RwLock<Splits>>) -> Self {
        Self {
            name,
            timer,
            splits,
        }
    }

    /// Get name of speedrun application (window title)
    pub fn get_name(&self) -> String {
        self.name.clone()
    }
}

#[derive(Default)]
struct Split {
    name: String,
    time: TimeSpan,
}

#[derive(Default)]
pub struct Splits {
    splits: Vec<Split>,
    name_padding: usize,
}

impl eframe::App for Speedrun {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let splits = self.splits.read().unwrap();
        let timespan = self
            .timer
            .read()
            .unwrap()
            .snapshot()
            .current_time()
            .real_time
            .unwrap();
        let current_time = format_timespan(timespan);
        let padding = splits.name_padding;
        let timer = self.timer.read().unwrap();
        let game_name = timer.run().game_name();
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading(game_name);
            for i in 0..splits.len() {
                ui.horizontal(|ui| {
                    ui.monospace(format!("{:<padding$}:", splits.get_split_name(i)));
                    ui.monospace(splits.get_timespan(i));
                });
            }
            ui.horizontal(|ui| {
                ui.monospace(format!("{:<padding$}:", "Time"));
                ui.monospace(current_time);
            });
        });

        // continously repaint even if out of focus
        ctx.request_repaint();
    }
}

impl Splits {
    pub fn new(split_names: Vec<String>) -> Splits {
        let mut splits: Vec<Split> = Vec::new();
        for name in split_names.clone() {
            splits.push(Split {
                name,
                time: TimeSpan::default(),
            });
        }

        Splits {
            splits,
            // padding for names of splits (= longest name)
            name_padding: split_names.into_iter().map(|n| n.len()).max().unwrap_or(0),
        }
    }

    /// Updates timespan for split i
    fn update_timespan(&mut self, i: usize, timespan: TimeSpan) {
        self.splits[i].time = timespan;
    }

    /// Get name of split i
    fn get_split_name(&self, i: usize) -> String {
        self.splits[i].name.clone()
    }

    /// Get timespan of split i
    fn get_timespan(&self, i: usize) -> String {
        self.splits[i].format_timespan()
    }

    /// Returns the number of splits
    fn len(&self) -> usize {
        self.splits.len()
    }
}

impl Split {
    /// Formats timespan to hh:mm:ss.ms
    fn format_timespan(&self) -> String {
        format_timespan(self.time)
    }
}

/// Formats timespan to hh:mm:ss.ms
pub fn format_timespan(timespan: TimeSpan) -> String {
    let d = timespan.to_duration();
    format!(
        "{:02}:{:02}:{:02}.{:03}",
        // TODO optionnal day/week formatting
        d.whole_hours().rem_euclid(24),
        d.whole_minutes().rem_euclid(60),
        d.whole_seconds().rem_euclid(60),
        d.whole_milliseconds().rem_euclid(1000)
    )
}

/// Starts the timer with the relevant keybinding and prints a message ("Start!" or "Split!")
pub fn start_or_split_timer(timer: Arc<RwLock<Timer>>, splits: Arc<RwLock<Splits>>) {
    let mut splits = splits.write().unwrap();
    let message = match timer.read().unwrap().current_phase() {
        NotRunning => "Start!",
        _ => "Split!",
    };
    println!("{message}");
    timer.write().unwrap().split_or_start();
    let timer = timer.read().unwrap();
    let snapshot = timer.snapshot();
    let segments = snapshot.run().segments();
    for (i, segment) in segments.iter().enumerate() {
        if let Some(timespan) = segment.split_time().real_time {
            splits.update_timespan(i, timespan);
        };
    }
}
