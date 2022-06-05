use eframe::egui;
use eframe::Storage;
use inputbot::KeybdKey;
use livesplit_core::TimeSpan;
use livesplit_core::Timer;
use livesplit_core::TimerPhase::*;
use log::{debug, error, info, warn};
use persistence::SpeedrunSettings;
use std::fmt;
use std::sync::{Arc, RwLock};
use strum::IntoEnumIterator;

pub mod persistence;

#[derive(Debug)]
pub enum Error {
    UI(String),
    Timer(String),
    UserInput(String),
    Other(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg: String = match self {
            Error::UI(msg) => format!("UI: {msg}"),
            Error::Timer(msg) => format!("Timer: {msg}"),
            Error::UserInput(msg) => format!("User input: {msg}"),
            Error::Other(msg) => format!("Other: {msg}"),
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
    comparison: TimeSpan,
    time_difference: TimeSpan,
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
        let current_time = match timer_readonly.snapshot().current_time().real_time {
            Some(ts) => ts,
            None => {
                warn!("Current time could not be parsed");
                TimeSpan::default()
            }
        };
        let current_time = format_timespan(current_time).unwrap();
        let padding = splits.name_padding;
        let run = timer_readonly.run();
        let category_name = run.category_name();
        let attempts_count = run.attempt_count();
        let comparison_name = timer_readonly.current_comparison();
        // truncate if too long
        let comparison_name = {
            if comparison_name.len() >= 13 {
                let (l, _) = comparison_name.split_at(10);
                let comparison_name = format!("{l}...");
                comparison_name
            } else {
                comparison_name.to_string()
            }
        };
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading(run.game_name());
            ui.monospace(format!("Category: {}", category_name));
            ui.monospace(format!("Attempts: {attempts_count}"));

            ui.horizontal(|ui| {
                ui.monospace(format!(
                    "{:<padding$}: Current time {:<13} Time difference",
                    "Splits", comparison_name
                ));
            });
            for i in 0..splits.len() {
                ui.horizontal(|ui| {
                    ui.monospace(format!("{:<padding$}:", splits.get_split_name(i)));
                    ui.monospace(splits.get_time(i).unwrap());
                    ui.monospace(splits.get_comparison(i).unwrap());
                    ui.monospace(splits.get_time_difference(i));
                });
            }
            ui.horizontal(|ui| {
                ui.monospace(format!("{:<padding$}:", "Time"));
                ui.monospace(current_time);
            });
            ui.monospace("");
            ui.monospace(format!("Start/split: {}", self.split_key));
            ui.monospace(format!("Reset      : {}", self.reset_key));
            ui.monospace("");
            ui.monospace("Note: attempts are saved when closing the application");
            ui.monospace("Note2: reset the timer for this attempt times to be stored in the run history when you close this application.");
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
                comparison: TimeSpan::default(),
                time_difference: TimeSpan::default(),
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

    /// Updates displayed split `i`
    pub fn update_split(&mut self, i: usize, time: TimeSpan, comparison: TimeSpan) {
        self.splits[i].time = time;
        // Comparison time gets filled at application start. When the timer
        // starts, the current segments does not know about the comparison time
        if comparison > TimeSpan::zero() {
            self.splits[i].comparison = comparison;
        }
        if time > TimeSpan::zero() {
            self.splits[i].time_difference = time - comparison;
        }
    }

    /// Reset display split
    fn clear_time_differences(&mut self) {
        for i in 0..self.splits.len() {
            self.splits[i].time_difference = TimeSpan::zero();
        }
    }

    /// Get name of split `i`
    fn get_split_name(&self, i: usize) -> String {
        self.splits[i].name.clone()
    }

    /// Get time of split `i`
    fn get_time(&self, i: usize) -> Result<String, Error> {
        format_timespan(self.splits[i].time)
    }

    /// Get formatted comparison of split `i`
    fn get_comparison(&self, i: usize) -> Result<String, Error> {
        format_timespan(self.splits[i].comparison)
    }

    /// Get formatted time difference with comparison of split `i`. '-'
    /// indicates a timesave
    fn get_time_difference(&self, i: usize) -> String {
        let time_difference = self.splits[i].time_difference;
        let sign = if time_difference.to_duration().is_negative() {
            '-'
        } else if time_difference.to_duration().is_positive() {
            '+'
        } else {
            ' '
        };
        format!("{}{}", sign, format_timespan_no_padding(time_difference))
    }

    /// Returns the number of splits
    fn len(&self) -> usize {
        self.splits.len()
    }
}

/// Formats timespan to "hh:mm:ss.ms". If calculating a potential timesave
/// (where time can be negative), use `format_timespan_no_padding` instead
pub fn format_timespan(time: TimeSpan) -> Result<String, Error> {
    let d = time.to_duration();
    if d.is_negative() {
        return Err(Error::Other(
            "Usage of negative time in function that only accepts positive time".to_string(),
        ));
    }
    Ok(format!(
        "{:02}:{:02}:{:02}.{:03}",
        // TODO optionnal day/week formatting
        d.whole_hours().rem_euclid(24),
        d.whole_minutes().rem_euclid(60),
        d.whole_seconds().rem_euclid(60),
        d.whole_milliseconds().rem_euclid(1000)
    ))
}

/// Formats timespan to hh:mm:ss.ms but does not display 00 values
pub fn format_timespan_no_padding(timespan: TimeSpan) -> String {
    let d = timespan.to_duration();
    let h = d.whole_hours() % 24;
    let m = d.whole_minutes() % 60;
    // NOTE: not using abs will print out '-' in case of a time save
    let s = format!(
        "{:02}:{:02}:{:02}.{:03}",
        // TODO optionnal day/week formatting
        h.abs(),
        m.abs(),
        (d.whole_seconds() % 60).abs(),
        (d.whole_milliseconds() % 1000).abs()
    );

    // always print seconds but remove minutes and hours if 0
    if h == 0 {
        let (_, l) = s.split_at(3);
        if m == 0 {
            let (_, l) = l.split_at(3);
            return l.to_string();
        }
        return l.to_string();
    }

    s
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
                let comparison = timer.current_comparison();
                let comparison = match segment.comparison(comparison).real_time {
                    Some(ts) => ts,
                    None => TimeSpan::default(),
                };
                debug!("{comparison:?}");
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
                if let Some(time) = segment.split_time().real_time {
                    splits_write.update_split(i, time, comparison);
                } else {
                    splits_write.update_split(i, TimeSpan::zero(), comparison);
                }
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

    // clear display
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

    // Update comparison time
    let run = timer.run();
    let comparison = timer.current_comparison();
    for (i, segment) in run.segments().iter().enumerate() {
        let comparison = match segment.comparison(comparison).real_time {
            Some(ts) => ts,
            None => TimeSpan::default(),
        };
        splits.update_split(i, TimeSpan::zero(), comparison);
    }
    splits.clear_time_differences();
}

/// Parse user key from string `key`
pub fn parse_key(key: String) -> Result<KeybdKey, Error> {
    KeybdKey::iter()
        .find(|k| format!("{:?}", k) == key)
        .ok_or(format!("Could not parse user key \"{key}\""))
        .map_err(Error::UserInput)
}
