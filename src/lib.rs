use eframe::egui;
use eframe::Storage;
use inputbot::KeybdKey;
use livesplit_core::TimeSpan;
use livesplit_core::Timer;
use livesplit_core::TimerPhase::*;
use log::{error, info, warn};
use persistence::SpeedrunSettings;
use std::fmt;
use std::sync::{Arc, RwLock};
use strum::IntoEnumIterator;

pub mod persistence;

pub enum Error {
    UI(String),
    Timer(String),
    UserInput(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg: String = match self {
            Error::UI(msg) => format!("UI: {msg}"),
            Error::Timer(msg) => format!("Timer: {msg}"),
            Error::UserInput(msg) => format!("User input: {msg}"),
        };
        write!(f, "{msg}")
    }
}

pub struct Speedrun {
    name: String,
    timer: Arc<RwLock<Timer>>,
    splits: Arc<RwLock<Splits>>,
    split_key: String,
    reset_key: String,
    settings: SpeedrunSettings,
}

impl Speedrun {
    pub fn new(
        name: String,
        timer: Arc<RwLock<Timer>>,
        splits: Arc<RwLock<Splits>>,
        split_key: KeybdKey,
        reset_key: KeybdKey,
        settings: SpeedrunSettings,
    ) -> Self {
        Self {
            name,
            timer,
            splits,
            split_key: format!("{:?}", split_key),
            reset_key: format!("{:?}", reset_key),
            settings,
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
        let splits = match self
            .splits
            .read()
            .map_err(|e| Error::UI(format!("splits mutex error: {e}")))
        {
            Ok(m) => m,
            Err(e) => {
                error!("{e}");
                panic!("{e}")
            }
        };
        let timer_readonly = match self
            .timer
            .read()
            .map_err(|e| Error::UI(format!("timer mutex error: {e}")))
        {
            Ok(m) => m,
            Err(e) => {
                error!("{e}");
                panic!("{e}")
            }
        };
        let timespan = match timer_readonly.snapshot().current_time().real_time {
            Some(ts) => ts,
            None => {
                warn!("Current time could not be parsed");
                TimeSpan::default()
            }
        };
        let current_time = format_timespan(timespan);
        let padding = splits.name_padding;
        let run = timer_readonly.run();
        let category_name = run.category_name();
        let attempts = run.attempt_count();
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading(run.game_name());
            ui.monospace(format!("Category: {}", category_name));
            ui.monospace(format!("Attempts: {attempts}"));
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
            ui.monospace("");
            ui.monospace(format!("Start/split: {}", self.split_key));
            ui.monospace(format!("Reset      : {}", self.reset_key));
        });

        // continously repaint even if out of focus
        ctx.request_repaint();
    }

    // NOTE: only called when persistence feature is enabled
    fn save(&mut self, _storage: &mut dyn Storage) {
        let timer = self.timer.read().unwrap();
        let run = timer.run();
        if let Err(e) = persistence::save_run_to_file(run, &self.settings) {
            error!("{e}");
        } else {
            info!("Saved run");
        }
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
            name_padding: split_names
                .into_iter()
                .map(|name| name.len())
                .max()
                .unwrap_or(0),
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

/// Starts the timer with the relevant keybinding and logs key press
pub fn start_or_split_timer(timer: Arc<RwLock<Timer>>, splits: Arc<RwLock<Splits>>) {
    let message = match timer.read().map_err(|e| {
        Error::Timer(format!(
            "Error with timer mutex while displaying timer start message: {e}"
        ))
    }) {
        Ok(timer) => match timer.current_phase() {
            NotRunning => "Start/split keypress: start",
            _ => "",
        },
        Err(e) => {
            error!("{e}");
            panic!("{e}")
        }
    };
    if !message.is_empty() {
        info!("{message}");
    }
    match timer
        .write()
        .map_err(|e| Error::Timer(format!("Error with timer mutex while splitting: {e}")))
    {
        Ok(mut timer) => timer.split_or_start(),
        Err(e) => {
            error!("{e}");
            panic!("{e}")
        }
    }
    match timer.read().map_err(|e| {
        Error::Timer(format!(
            "Error while capturing snapshot of run with timer mutex: {e}"
        ))
    }) {
        Ok(timer) => {
            let snapshot = timer.snapshot();
            let segments = snapshot.run().segments();
            for (i, segment) in segments.iter().enumerate() {
                if let Some(timespan) = segment.split_time().real_time {
                    let mut splits_write = match splits
                        .write()
                        .map_err(|e| Error::Timer(format!("splits mutex error: {e}")))
                    {
                        Ok(m) => m,
                        Err(e) => {
                            error!("{e}");
                            panic!("{e}")
                        }
                    };
                    splits_write.update_timespan(i, timespan);
                };
            }

            // if timer was started, don't check for splits or speedrun end
            if message.is_empty() {
                let message = match timer.current_phase() {
                    Ended => "Ended!",
                    _ => "Start/split keypress: split",
                };
                info!("{message}");
            }
        }
        Err(e) => {
            error!("{e}");
            panic!("{e}")
        }
    };
}

/// Reset the timer (which adds one attempt) and clear splits time
pub fn reset(timer: Arc<RwLock<Timer>>, splits: Arc<RwLock<Splits>>) {
    info!("Reset keypress");
    let mut splits = match splits
        .write()
        .map_err(|e| Error::Timer(format!("splits mutex error: {e}")))
    {
        Ok(m) => m,
        Err(e) => {
            error!("{e}");
            panic!("{e}")
        }
    };
    for i in 0..splits.len() {
        splits.update_timespan(i, TimeSpan::zero());
    }

    let mut timer = match timer
        .write()
        .map_err(|e| Error::Timer(format!("timer mutex error: {e}")))
    {
        Ok(m) => m,
        Err(e) => {
            error!("{e}");
            panic!("{e}")
        }
    };
    timer.reset(true);
}

/// Parse user key from string `key`
pub fn parse_key(key: String) -> Result<KeybdKey, Error> {
    KeybdKey::iter()
        .find(|k| format!("{:?}", k) == format!("{key}"))
        .ok_or(format!("Could not parse user key \"{key}\""))
        .map_err(|e| Error::UserInput(format!("{e}")))
}
