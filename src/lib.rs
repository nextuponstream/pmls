#![deny(missing_docs)]
#![deny(rustdoc::invalid_codeblock_attributes)]
#![warn(rustdoc::bare_urls)]
#![deny(rustdoc::broken_intra_doc_links)]
//! speedrun_splits is an application that lets you time your speedrun.
//!
//! IMPORTANT NOTICE: This crate is is not meant to replace the official livesplit client that might come to linux
//! once finished. Official desktop MVP state is detailed [here](https://github.com/LiveSplit/livesplit-core/projects/2).
//!
//! speedrun_splits allows you to be in game and use your keyboard to time your
//! speedruns using the livesplit_core library.

use eframe::egui;
use eframe::Storage;
use egui_extras::RetainedImage;
use livesplit_core::hotkey::KeyCode;
use livesplit_core::TimeSpan;
use livesplit_core::Timer;
use livesplit_core::TimerPhase::*;
use log::{debug, error, info, warn};
use persistence::SpeedrunSettings;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fmt::Debug;
use std::sync::{Arc, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard};

pub mod persistence;

#[derive(Debug)]
/// General errors that may happen while using speedrun_splits
pub enum Error<'a> {
    /// Unrecoverable error with the timer
    TimerWriteLock(PoisonError<RwLockWriteGuard<'a, livesplit_core::Timer>>),
    /// Unrecoverable error with the timer
    TimerReadLock(PoisonError<RwLockReadGuard<'a, livesplit_core::Timer>>),
    /// Unrecoverable error with the timer
    SplitsReadLock(PoisonError<RwLockReadGuard<'a, Splits>>),
    /// Unrecoverable error with the splits display
    SplitsWriteLock(PoisonError<RwLockWriteGuard<'a, Splits>>),
    /// Input from user is invalid
    UserInput(String),
    /// Unrecoverable error such as division by zero
    Other(String),
}

impl From<()> for Error<'_> {
    fn from(_: ()) -> Self {
        Error::Other("Could not convert key".to_string())
    }
}

impl<'a> From<std::string::String> for Error<'a> {
    fn from(e: std::string::String) -> Self {
        Error::Other(e)
    }
}

impl<'a> From<PoisonError<RwLockReadGuard<'a, livesplit_core::Timer>>> for Error<'a> {
    fn from(e: PoisonError<RwLockReadGuard<'a, livesplit_core::Timer>>) -> Self {
        Error::TimerReadLock(e)
    }
}

impl fmt::Display for Error<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::TimerWriteLock(lock) => fmt::Display::fmt(lock, f),
            Error::TimerReadLock(lock) => fmt::Display::fmt(lock, f),
            Error::SplitsReadLock(lock) => fmt::Display::fmt(lock, f),
            Error::SplitsWriteLock(lock) => fmt::Display::fmt(lock, f),
            Error::UserInput(msg) => writeln!(f, "{msg}"),
            Error::Other(msg) => writeln!(f, "Other: {msg}"),
        }
    }
}

/// Speedrun and all its associated settings
pub struct Speedrun {
    name: String,
    timer: Arc<RwLock<Timer>>,
    splits: Arc<RwLock<Splits>>,
    split_key: String,
    reset_key: String,
    pause_key: String,
    unpause_key: String,
    comparison_key: String,
    settings: SpeedrunSettings,
    icons: Vec<RetainedImage>,
}

/// Effective keybindings in use for speedrun
#[derive(Serialize, Deserialize)]
pub struct Keybinding {
    split_key: KeyCode,
    reset_key: KeyCode,
    pause_key: KeyCode,
    unpause_key: KeyCode,
    comparison_key: KeyCode,
}

#[derive(Default)]
struct Split {
    name: String,
    time: TimeSpan,
    comparison: TimeSpan,
    time_difference: TimeSpan,
}

#[derive(Default)]
/// Time splits of a speedrun
pub struct Splits {
    splits: Vec<Split>,
    name_padding: usize,
}

impl Keybinding {
    /// Return Keybinding for speedrun_splits application
    pub fn new(
        split_key: KeyCode,
        reset_key: KeyCode,
        pause_key: KeyCode,
        unpause_key: KeyCode,
        comparison_key: KeyCode,
    ) -> Keybinding {
        Keybinding {
            split_key,
            reset_key,
            pause_key,
            unpause_key,
            comparison_key,
        }
    }
}

impl Speedrun {
    /// Return Speedrun for speedrun_splits application
    pub fn new(
        name: String,
        timer: Arc<RwLock<Timer>>,
        splits: Arc<RwLock<Splits>>,
        keybinding: Keybinding,
        settings: SpeedrunSettings,
    ) -> Self {
        Self {
            name,
            timer,
            splits,
            split_key: format!("{:?}", keybinding.split_key),
            reset_key: format!("{:?}", keybinding.reset_key),
            pause_key: format!("{:?}", keybinding.pause_key),
            unpause_key: format!("{:?}", keybinding.unpause_key),
            comparison_key: format!("{:?}", keybinding.comparison_key),
            settings,
            icons: vec![],
        }
    }

    /// Get name of speedrun application (window title)
    pub fn get_name(&self) -> String {
        self.name.clone()
    }

    /// Load speedrun icons if present
    ///
    /// NOTE: loading image images in update loop is costly for the CPU.
    ///       Loading in init function makes cpu cost go from 30% -> 5% (top)
    pub fn init(&mut self) -> Result<(), Error> {
        info!("preloading speedrun icons...");
        let timer = self.timer.read()?;
        for segment in timer.run().segments() {
            let img_data = segment.icon().data();
            if !img_data.is_empty() {
                let image = RetainedImage::from_image_bytes(segment.name(), img_data)?;
                self.icons.push(image);
            }
        }

        Ok(())
    }
}

impl eframe::App for Speedrun {
    // NOTE: obtaining a write lock inside the update function does not work.
    //       The workaround is to bind a key to a callback function.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let splits = match self.splits.read().map_err(Error::SplitsReadLock) {
            Ok(m) => m,
            Err(e) => {
                error!("{e}");
                panic!("{e}") // cannot recover
            }
        };
        let timer_readonly = match self.timer.read().map_err(Error::TimerReadLock) {
            Ok(m) => m,
            Err(e) => {
                error!("{e}");
                panic!("{e}") // cannot recover
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
                let image_padding = run_has_icon(run);
                ui.monospace(format!(
                    "{}{:<padding$}: Current time {:<13} Time difference",
                    image_padding, "Splits", comparison_name
                ));
            });
            for i in 0..splits.len() {
                ui.horizontal(|ui| {
                    // example: https://github.com/emilk/egui/blob/0.17.0/eframe/examples/image.rs
                    if let Some(img) = self.icons.get(i) {
                        // 27 pixels is ~= 5 whitespaces
                        let dimensions = egui::Vec2::new(27f32,27f32);
                        //ui.image(image.texture_id(ctx), image.size_vec2());
                        ui.image(img.texture_id(ctx), dimensions);
                    }
                    ui.monospace(format!("{:<padding$}:", splits.get_split_name(i)));
                    ui.monospace(splits.get_time(i).unwrap());
                    ui.monospace(splits.get_comparison(i).unwrap());
                    ui.monospace(splits.get_time_difference(i));
                });
            }
            ui.horizontal(|ui| {
                let image_padding = run_has_icon(run);
                ui.monospace(format!("{}{:<padding$}:", image_padding, "Time"));
                ui.monospace(current_time);
            });
            ui.monospace("");
            ui.monospace(format!("Start/split      : {}", self.split_key));
            ui.monospace(format!("Reset            : {}", self.reset_key));
            ui.monospace(format!("Pause            : {}", self.pause_key));
            ui.monospace(format!("Unpause          : {}", self.unpause_key));
            ui.monospace(format!("Switch comparison: {}", self.comparison_key));
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
    /// create [Splits](Splits) items from `split_names`
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
        let time_difference = format_timesave(time_difference);
        format!("{sign}{time_difference}")
    }

    /// Returns the number of splits
    fn len(&self) -> usize {
        self.splits.len()
    }

    /// Refresh splits display (called right after switching comparison)
    pub fn refresh_splits(&mut self, i: usize, comparison: TimeSpan) {
        self.splits[i].comparison = comparison;
        if self.splits[i].time > TimeSpan::zero() {
            self.splits[i].time_difference = self.splits[i].time - self.splits[i].comparison;
        }
    }
}

/// Formats `timespan` to "hh:mm:ss.ms"
fn format_timespan<'a>(time: TimeSpan) -> Result<String, Error<'a>> {
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

/// Format `timespan` to hh:mm:ss.ms and shorten displayed text when possible
fn format_timesave(timespan: TimeSpan) -> String {
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

/// Starts `timer`, logs keypress and update `splits` display
pub fn start_or_split_timer(timer: Arc<RwLock<Timer>>, splits: Arc<RwLock<Splits>>) {
    let message = match timer.read().map_err(Error::TimerReadLock) {
        Ok(timer) => match timer.current_phase() {
            NotRunning => "Start/split keypress: start",
            _ => "",
        },
        Err(e) => {
            error!("{e}");
            panic!("{e}") // cannot recover
        }
    };
    if !message.is_empty() {
        info!("{message}");
    }
    match timer.write().map_err(Error::TimerWriteLock) {
        Ok(mut timer) => timer.split_or_start(),
        Err(e) => {
            error!("{e}");
            panic!("{e}") // cannot recover
        }
    }
    match timer.read().map_err(Error::TimerReadLock) {
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
                let mut splits_write = match splits.write().map_err(Error::SplitsWriteLock) {
                    Ok(m) => m,
                    Err(e) => {
                        error!("{e}");
                        panic!("{e}") // cannot recover
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
            panic!("{e}") // cannot recover
        }
    };
}

/// Reset `timer` (which adds one attempt) and clear `splits` time display
pub fn reset(timer: Arc<RwLock<Timer>>, splits: Arc<RwLock<Splits>>) {
    info!("Reset keypress");
    let mut timer = match timer.write().map_err(Error::TimerWriteLock) {
        Ok(m) => m,
        Err(e) => {
            error!("{e}");
            panic!("{e}") // cannot recover
        }
    };
    timer.reset(true);

    // clear display
    let mut splits = match splits.write().map_err(Error::SplitsWriteLock) {
        Ok(m) => m,
        Err(e) => {
            error!("{e}");
            panic!("{e}") // cannot recover
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

/// Pause `timer`
pub fn pause(timer: Arc<RwLock<Timer>>) {
    info!("timer paused");
    match timer.write().map_err(Error::TimerWriteLock) {
        Ok(mut timer) => {
            timer.pause();
        }
        Err(e) => {
            error!("{e}");
            panic!("{e}") // cannot recover
        }
    }
}

/// Unpause `timer`
///
/// Uses the resume method of the timer
pub fn unpause(timer: Arc<RwLock<Timer>>) {
    info!("timer resumed");
    match timer.write().map_err(Error::TimerWriteLock) {
        Ok(mut timer) => {
            timer.resume();
        }
        Err(e) => {
            error!("{e}");
            panic!("{e}") // cannot recover
        }
    }
}

/// Switch to next comparison
pub fn switch_comparison(timer: Arc<RwLock<Timer>>, splits: Arc<RwLock<Splits>>) {
    info!("Switching comparison");
    let mut timer = match timer.write().map_err(Error::TimerWriteLock) {
        Ok(timer) => timer,
        Err(e) => {
            error!("{e}");
            panic!("{e}") // cannot recover
        }
    };
    timer.switch_to_next_comparison();

    let snapshot = timer.snapshot();
    let segments = snapshot.run().segments();
    for (i, segment) in segments.iter().enumerate() {
        let comparison = timer.current_comparison();
        let comparison = match segment.comparison(comparison).real_time {
            Some(ts) => ts,
            None => TimeSpan::default(),
        };
        let mut splits_write = match splits.write().map_err(Error::SplitsWriteLock) {
            Ok(m) => m,
            Err(e) => {
                error!("{e}");
                panic!("{e}") // cannot recover
            }
        };
        splits_write.refresh_splits(i, comparison);
    }
}

/// Parse `key`
pub fn parse_key<'a>(key: String) -> Result<KeyCode, Error<'a>> {
    Ok(key.parse::<KeyCode>()?)
}

/// Returns true if splits have icons to display
fn run_has_icon(run: &livesplit_core::Run) -> &str {
    let img_data = run.segment(0).icon().data();
    if img_data.is_empty() {
        ""
    } else {
        "     "
    }
}
