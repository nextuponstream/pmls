//! Control the timer with keybinding and its callback functions
use crate::ui::Splits;
use livesplit_core::TimeSpan;
use livesplit_core::Timer;
use livesplit_core::TimerPhase::{Ended, NotRunning};
use log::{error, info};
use std::fmt;
use std::fmt::Debug;
use std::sync::{Arc, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard};

/// Errors while controlling the timer with keybindings
#[derive(Debug)]
pub enum TimerControlError<'a> {
    /// Unrecoverable error with the timer
    TimerWriteLock(PoisonError<RwLockWriteGuard<'a, livesplit_core::Timer>>),
    /// Unrecoverable error with the timer
    TimerReadLock(PoisonError<RwLockReadGuard<'a, livesplit_core::Timer>>),
    /// Unrecoverable error with the timer
    SplitsReadLock(PoisonError<RwLockReadGuard<'a, Splits>>),
    /// Unrecoverable error with the splits display
    SplitsWriteLock(PoisonError<RwLockWriteGuard<'a, Splits>>),
}

impl fmt::Display for TimerControlError<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TimerControlError::TimerWriteLock(lock) => fmt::Display::fmt(lock, f),
            TimerControlError::TimerReadLock(lock) => fmt::Display::fmt(lock, f),
            TimerControlError::SplitsReadLock(lock) => fmt::Display::fmt(lock, f),
            TimerControlError::SplitsWriteLock(lock) => fmt::Display::fmt(lock, f),
        }
    }
}

/// Starts `timer`, logs keypress and update `splits` display
///
/// # Panics
/// Panics if any errors occur with the timer or the splits display
pub fn start_or_split_timer(timer: &Arc<RwLock<Timer>>, splits: &Arc<RwLock<Splits>>) {
    let message = match timer.read().map_err(TimerControlError::TimerReadLock) {
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
    match timer.write().map_err(TimerControlError::TimerWriteLock) {
        Ok(mut timer) => timer.split_or_start(),
        Err(e) => {
            error!("{e}");
            panic!("{e}") // cannot recover
        }
    }
    match timer.read().map_err(TimerControlError::TimerReadLock) {
        Ok(timer) => {
            let snapshot = timer.snapshot();
            let segments = snapshot.run().segments();
            for (i, segment) in segments.iter().enumerate() {
                let comparison = timer.current_comparison();
                let comparison = match segment.comparison(comparison).real_time {
                    Some(ts) => ts,
                    None => TimeSpan::default(),
                };
                let mut splits_write =
                    match splits.write().map_err(TimerControlError::SplitsWriteLock) {
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
///
/// # Panics
/// Panics if any errors occur with the timer or the splits display
pub fn reset(timer: &Arc<RwLock<Timer>>, splits: &Arc<RwLock<Splits>>) {
    info!("Reset keypress");
    let mut timer = match timer.write().map_err(TimerControlError::TimerWriteLock) {
        Ok(m) => m,
        Err(e) => {
            error!("{e}");
            panic!("{e}") // cannot recover
        }
    };
    timer.reset(true);

    // clear display
    let mut splits = match splits.write().map_err(TimerControlError::SplitsWriteLock) {
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
///
/// # Panics
/// Panics if any errors occur with the timer or the splits display
pub fn pause(timer: &Arc<RwLock<Timer>>) {
    info!("timer paused");
    match timer.write().map_err(TimerControlError::TimerWriteLock) {
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
///
/// # Panics
/// Panics if any errors occur with the timer or the splits display
pub fn unpause(timer: &Arc<RwLock<Timer>>) {
    info!("timer resumed");
    match timer.write().map_err(TimerControlError::TimerWriteLock) {
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
///
/// # Panics
/// Panics if any errors occur with the timer or the splits display
pub fn switch_comparison(timer: &Arc<RwLock<Timer>>, splits: &Arc<RwLock<Splits>>) {
    info!("Switching comparison");
    let mut timer = match timer.write().map_err(TimerControlError::TimerWriteLock) {
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
        let mut splits_write = match splits.write().map_err(TimerControlError::SplitsWriteLock) {
            Ok(m) => m,
            Err(e) => {
                error!("{e}");
                panic!("{e}") // cannot recover
            }
        };
        splits_write.refresh_splits(i, comparison);
    }
}
