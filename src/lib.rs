#![deny(missing_docs)]
#![deny(rustdoc::invalid_codeblock_attributes)]
#![warn(rustdoc::bare_urls)]
#![deny(rustdoc::broken_intra_doc_links)]
#![doc = include_str!("../README.md")]
#![warn(clippy::pedantic)]
pub mod persistence;
pub mod timer_controls;
pub mod ui;

use livesplit_core::hotkey::KeyCode;
use serde::{Deserialize, Serialize};

/// Effective keybindings in use for speedrun
#[derive(Serialize, Deserialize, Copy, Clone)]
pub struct Keybinding {
    split_key: KeyCode,
    reset_key: KeyCode,
    pause_key: KeyCode,
    unpause_key: KeyCode,
    comparison_key: KeyCode,
}

impl Keybinding {
    /// Return Keybinding for the application
    #[must_use]
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
