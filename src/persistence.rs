//! Persist user files for the speedrun_splits application
//!
//! * run file (.lss)
//! * settings (.txt) associated with speedrun
//! * general configuration (`$HOME/.config/.speedrun_splits`)
//! * log file

use crate::{parse_key, Keybinding};
use crate::{Error as lError, Splits};
use dialog::{DialogBox, Input};
use itertools::Itertools;
use livesplit_core::hotkey::KeyCode;
use livesplit_core::hotkey::KeyCode::{Numpad1, Numpad3, Numpad5, Numpad7, Numpad9};
use livesplit_core::run::parser::composite;
use livesplit_core::run::saver::livesplit;
use livesplit_core::Run;
use log::*;
use serde::{Deserialize, Serialize};
use std::env::VarError;
use std::fmt::Debug;
use std::fs::File;
use std::io::BufReader;
use std::io::BufWriter;
use std::io::{Read, Write};
use std::path::Path;
use std::path::PathBuf;
use std::sync::PoisonError;
use std::{fmt, fs};
use walkdir::WalkDir;

#[derive(Serialize, Deserialize, Clone)]
/// Configuration file of the speedrun_splits application
pub struct SpeedrunSplitsConfiguration {
    data_folder_path: String,
    // open default speedrun
    use_default_speedrun: bool,
    default_speedrun_name: Option<String>,
}

#[derive(Serialize, Deserialize)]
/// Settings for speedrun
pub struct SpeedrunSettings {
    split_names: Vec<String>,
    game_name: String,
    category_name: String,
    keybindings: Keybinding,
}

/// Keybinding provided by the user from cli args
pub struct UserKeybinding<'a> {
    split_key: Option<&'a str>,
    reset_key: Option<&'a str>,
    pause_key: Option<&'a str>,
    unpause_key: Option<&'a str>,
    comparison_key: Option<&'a str>,
}

#[derive(Debug)]
/// Errors while persisting necessary files for the speedrun_splits application
pub enum Error<'a> {
    /// Input from user is invalid
    UserInput(String),
    /// Error with the user environment variables
    VarError(VarError),
    /// User cannot interact with dialog box
    Dialog(dialog::Error),
    /// Error with filesystem
    IO(std::io::Error),
    /// Unrecoverable error with timer
    TimerWriteLock(PoisonError<std::sync::RwLockWriteGuard<'a, livesplit_core::Timer>>),
    /// Unrecoverable error with timer
    TimerReadLock(PoisonError<std::sync::RwLockReadGuard<'a, livesplit_core::Timer>>),
    /// Unrecoverable error with splits display
    SplitsReadLock(PoisonError<std::sync::RwLockReadGuard<'a, Splits>>),
    /// Unrecoverable error with splits display
    SplitsWriteLock(PoisonError<std::sync::RwLockWriteGuard<'a, Splits>>),
    /// Unrecoverable error such as division by zero
    Other(String),
}

/// Errors while using the configuration file of the speedrun_splits application
pub enum SSConfigurationFileError<'a> {
    /// Input from user is invalid
    UserInput(String),
    /// Error with the user environment variables
    VarError(VarError),
    /// User cannot interact with dialog box
    Dialog(dialog::Error),
    /// Error with filesystem
    IO(std::io::Error),
    /// Error while parsing user's speedrun_splits data folder
    DataFolder(walkdir::Error),
    /// Serialization to toml format error
    Serialize(toml::ser::Error),
    /// Deserialization from toml format error
    Deserialize(toml::de::Error),
    /// Unrecoverable error with timer
    TimerWriteLock(PoisonError<std::sync::RwLockWriteGuard<'a, livesplit_core::Timer>>),
    /// Unrecoverable error with timer
    TimerReadLock(PoisonError<std::sync::RwLockReadGuard<'a, livesplit_core::Timer>>),
    /// Unrecoverable error with splits display
    SplitsReadLock(PoisonError<std::sync::RwLockReadGuard<'a, Splits>>),
    /// Unrecoverable error with splits display
    SplitsWriteLock(PoisonError<std::sync::RwLockWriteGuard<'a, Splits>>),
    /// Unrecoverable error such as division by zero
    Other(String),
}

/// Errors while using the settings file of a speedrun
pub enum SpeedrunSettingsFileError<'a> {
    /// Input from user is invalid
    UserInput(String),
    /// Serialization to toml format error
    Serialize(toml::ser::Error),
    /// Deserialization from toml format error
    Deserialize(toml::de::Error),
    /// Conversion of OS string for filepath
    OSStringConversion(String),
    /// Error with the user environment variables
    VarError(VarError),
    /// User cannot interact with dialog box
    Dialog(dialog::Error),
    /// Error with filesystem
    IO(std::io::Error),
    /// Unrecoverable error with timer
    TimerWriteLock(PoisonError<std::sync::RwLockWriteGuard<'a, livesplit_core::Timer>>),
    /// Unrecoverable error with timer
    TimerReadLock(PoisonError<std::sync::RwLockReadGuard<'a, livesplit_core::Timer>>),
    /// Unrecoverable error with splits display
    SplitsReadLock(PoisonError<std::sync::RwLockReadGuard<'a, Splits>>),
    /// Unrecoverable error with splits display
    SplitsWriteLock(PoisonError<std::sync::RwLockWriteGuard<'a, Splits>>),
    /// Unrecoverable error such as division by zero
    Other(String),
}

/// Errors while using the run file of a speedrun
pub enum RunFileError<'a> {
    /// Input from user is invalid
    UserInput(String),
    /// Cannot save `.lss` file
    Save(livesplit_core::run::saver::livesplit::Error),
    /// Cannot parse `.lss` file
    Parse(livesplit_core::run::parser::composite::Error),
    /// Error with the user environment variables
    VarError(VarError),
    /// User cannot interact with dialog box
    Dialog(dialog::Error),
    /// Error with filesystem
    IO(std::io::Error),
    /// Unrecoverable error with timer
    TimerWriteLock(PoisonError<std::sync::RwLockWriteGuard<'a, livesplit_core::Timer>>),
    /// Unrecoverable error with timer
    TimerReadLock(PoisonError<std::sync::RwLockReadGuard<'a, livesplit_core::Timer>>),
    /// Unrecoverable error with splits display
    SplitsReadLock(PoisonError<std::sync::RwLockReadGuard<'a, Splits>>),
    /// Unrecoverable error with splits display
    SplitsWriteLock(PoisonError<std::sync::RwLockWriteGuard<'a, Splits>>),
    /// Unrecoverable error such as division by zero
    Other(String),
}

impl<'a> SpeedrunSettings {
    fn new(
        split_names: Vec<String>,
        game_name: String,
        category_name: String,
        keybindings: Keybinding,
    ) -> Result<SpeedrunSettings, SpeedrunSettingsFileError<'a>> {
        let keys = vec![
            keybindings.split_key,
            keybindings.reset_key,
            keybindings.pause_key,
            keybindings.unpause_key,
        ];
        if !keys.iter().all_unique() {
            return Err(SpeedrunSettingsFileError::UserInput(
                "All keys need to be bound to a different key".to_string(),
            ));
        }
        Ok(SpeedrunSettings {
            split_names,
            game_name,
            category_name,
            keybindings,
        })
    }
}

impl<'a> SpeedrunSplitsConfiguration {
    fn new() -> Result<SpeedrunSplitsConfiguration, SSConfigurationFileError<'a>> {
        Ok(SpeedrunSplitsConfiguration {
            data_folder_path: default_data_folder()?,
            default_speedrun_name: None,
            use_default_speedrun: true,
        })
    }
}

impl<'a> fmt::Display for SSConfigurationFileError<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SSConfigurationFileError::UserInput(msg) => writeln!(f, "{msg}"),
            SSConfigurationFileError::Dialog(de) => fmt::Display::fmt(de, f),
            SSConfigurationFileError::IO(ioe) => fmt::Display::fmt(ioe, f),
            SSConfigurationFileError::DataFolder(wde) => fmt::Display::fmt(wde, f),
            SSConfigurationFileError::Serialize(se) => fmt::Display::fmt(se, f),
            SSConfigurationFileError::Deserialize(de) => fmt::Display::fmt(de, f),
            SSConfigurationFileError::TimerReadLock(lock) => fmt::Display::fmt(lock, f),
            SSConfigurationFileError::TimerWriteLock(lock) => fmt::Display::fmt(lock, f),
            SSConfigurationFileError::SplitsReadLock(lock) => fmt::Display::fmt(lock, f),
            SSConfigurationFileError::SplitsWriteLock(lock) => fmt::Display::fmt(lock, f),
            SSConfigurationFileError::VarError(v) => fmt::Display::fmt(v, f),
            SSConfigurationFileError::Other(msg) => writeln!(f, "{msg}"),
        }
    }
}

impl<'a> From<std::io::Error> for Error<'a> {
    fn from(e: std::io::Error) -> Self {
        Error::IO(e)
    }
}

impl<'a> From<VarError> for Error<'a> {
    fn from(e: VarError) -> Self {
        Error::VarError(e)
    }
}

impl<'a> From<lError<'a>> for Error<'a> {
    fn from(e: lError<'a>) -> Self {
        match e {
            lError::UserInput(msg) => Error::UserInput(msg),
            lError::TimerWriteLock(lock) => Error::TimerWriteLock(lock),
            lError::TimerReadLock(lock) => Error::TimerReadLock(lock),
            lError::SplitsReadLock(lock) => Error::SplitsReadLock(lock),
            lError::SplitsWriteLock(lock) => Error::SplitsWriteLock(lock),
            lError::Other(msg) => Error::Other(msg),
        }
    }
}

impl<'a> From<VarError> for SSConfigurationFileError<'a> {
    fn from(e: VarError) -> Self {
        SSConfigurationFileError::VarError(e)
    }
}

impl<'a> From<dialog::Error> for SSConfigurationFileError<'a> {
    fn from(e: dialog::Error) -> Self {
        SSConfigurationFileError::Dialog(e)
    }
}

impl<'a> From<std::io::Error> for RunFileError<'a> {
    fn from(e: std::io::Error) -> Self {
        RunFileError::IO(e)
    }
}

impl<'a> From<Error<'a>> for RunFileError<'a> {
    fn from(e: Error<'a>) -> Self {
        match e {
            Error::UserInput(msg) => RunFileError::UserInput(msg),
            Error::VarError(ve) => RunFileError::VarError(ve),
            Error::Dialog(de) => RunFileError::Dialog(de),
            Error::IO(io) => RunFileError::IO(io),
            Error::TimerWriteLock(lock) => RunFileError::TimerWriteLock(lock),
            Error::TimerReadLock(lock) => RunFileError::TimerReadLock(lock),
            Error::SplitsReadLock(lock) => RunFileError::SplitsReadLock(lock),
            Error::SplitsWriteLock(lock) => RunFileError::SplitsWriteLock(lock),
            Error::Other(msg) => RunFileError::Other(msg),
        }
    }
}

impl<'a> From<livesplit_core::run::parser::composite::Error> for RunFileError<'a> {
    fn from(e: livesplit_core::run::parser::composite::Error) -> Self {
        RunFileError::Parse(e)
    }
}

impl<'a> From<livesplit_core::run::saver::livesplit::Error> for RunFileError<'a> {
    fn from(e: livesplit_core::run::saver::livesplit::Error) -> Self {
        RunFileError::Save(e)
    }
}

impl<'a> From<std::io::Error> for SSConfigurationFileError<'a> {
    fn from(io: std::io::Error) -> Self {
        SSConfigurationFileError::IO(io)
    }
}

impl<'a> From<Error<'a>> for SSConfigurationFileError<'a> {
    fn from(e: Error<'a>) -> Self {
        match e {
            Error::UserInput(msg) => SSConfigurationFileError::UserInput(msg),
            Error::Dialog(de) => SSConfigurationFileError::Dialog(de),
            Error::IO(io) => SSConfigurationFileError::IO(io),
            Error::VarError(ve) => SSConfigurationFileError::VarError(ve),
            Error::TimerReadLock(lock) => SSConfigurationFileError::TimerReadLock(lock),
            Error::TimerWriteLock(lock) => SSConfigurationFileError::TimerWriteLock(lock),
            Error::SplitsReadLock(lock) => SSConfigurationFileError::SplitsReadLock(lock),
            Error::SplitsWriteLock(lock) => SSConfigurationFileError::SplitsWriteLock(lock),
            Error::Other(msg) => SSConfigurationFileError::Other(msg),
        }
    }
}

impl<'a> From<toml::de::Error> for SpeedrunSettingsFileError<'a> {
    fn from(e: toml::de::Error) -> Self {
        SpeedrunSettingsFileError::Deserialize(e)
    }
}

impl<'a> From<toml::ser::Error> for SSConfigurationFileError<'a> {
    fn from(e: toml::ser::Error) -> Self {
        SSConfigurationFileError::Serialize(e)
    }
}

impl<'a> From<toml::ser::Error> for SpeedrunSettingsFileError<'a> {
    fn from(e: toml::ser::Error) -> Self {
        SpeedrunSettingsFileError::Serialize(e)
    }
}

impl<'a> From<std::io::Error> for SpeedrunSettingsFileError<'a> {
    fn from(e: std::io::Error) -> Self {
        SpeedrunSettingsFileError::IO(e)
    }
}

impl<'a> From<&str> for SpeedrunSettingsFileError<'a> {
    fn from(e: &str) -> Self {
        SpeedrunSettingsFileError::OSStringConversion(e.to_string())
    }
}

impl<'a> From<walkdir::Error> for SSConfigurationFileError<'a> {
    fn from(e: walkdir::Error) -> Self {
        SSConfigurationFileError::DataFolder(e)
    }
}

impl<'a> From<SSConfigurationFileError<'a>> for std::fmt::Error {
    fn from(e: SSConfigurationFileError) -> Self {
        // log error before losing information
        error!("{e}");
        std::fmt::Error
    }
}

impl<'a> From<Error<'a>> for std::fmt::Error {
    fn from(e: Error) -> Self {
        // log error before losing information
        error!("{e}");
        std::fmt::Error
    }
}

impl<'a> From<lError<'a>> for SpeedrunSettingsFileError<'a> {
    fn from(e: lError<'a>) -> Self {
        match e {
            lError::UserInput(msg) => SpeedrunSettingsFileError::UserInput(msg),
            lError::TimerWriteLock(lock) => SpeedrunSettingsFileError::TimerWriteLock(lock),
            lError::TimerReadLock(lock) => SpeedrunSettingsFileError::TimerReadLock(lock),
            lError::SplitsReadLock(lock) => SpeedrunSettingsFileError::SplitsReadLock(lock),
            lError::SplitsWriteLock(lock) => SpeedrunSettingsFileError::SplitsWriteLock(lock),
            lError::Other(msg) => SpeedrunSettingsFileError::Other(msg),
        }
    }
}

impl<'a> From<Error<'a>> for SpeedrunSettingsFileError<'a> {
    fn from(e: Error<'a>) -> Self {
        match e {
            Error::Dialog(de) => SpeedrunSettingsFileError::Dialog(de),
            Error::UserInput(msg) => SpeedrunSettingsFileError::UserInput(msg),
            Error::IO(io) => SpeedrunSettingsFileError::IO(io),
            Error::VarError(ve) => SpeedrunSettingsFileError::VarError(ve),
            Error::TimerReadLock(lock) => SpeedrunSettingsFileError::TimerReadLock(lock),
            Error::TimerWriteLock(lock) => SpeedrunSettingsFileError::TimerWriteLock(lock),
            Error::SplitsReadLock(lock) => SpeedrunSettingsFileError::SplitsReadLock(lock),
            Error::SplitsWriteLock(lock) => SpeedrunSettingsFileError::SplitsWriteLock(lock),
            Error::Other(msg) => SpeedrunSettingsFileError::Other(msg),
        }
    }
}

impl<'a> From<toml::de::Error> for SSConfigurationFileError<'a> {
    fn from(e: toml::de::Error) -> Self {
        SSConfigurationFileError::Deserialize(e)
    }
}

impl<'a> fmt::Display for SpeedrunSettingsFileError<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SpeedrunSettingsFileError::UserInput(msg) => writeln!(f, "{msg}"),
            SpeedrunSettingsFileError::OSStringConversion(msg) => writeln!(f, "{msg}"),
            SpeedrunSettingsFileError::VarError(ve) => fmt::Display::fmt(ve, f),
            SpeedrunSettingsFileError::Dialog(de) => fmt::Display::fmt(de, f),
            SpeedrunSettingsFileError::Serialize(se) => fmt::Display::fmt(se, f),
            SpeedrunSettingsFileError::Deserialize(de) => fmt::Display::fmt(de, f),
            SpeedrunSettingsFileError::IO(ioe) => fmt::Display::fmt(ioe, f),
            SpeedrunSettingsFileError::TimerReadLock(lock) => fmt::Display::fmt(lock, f),
            SpeedrunSettingsFileError::TimerWriteLock(lock) => fmt::Display::fmt(lock, f),
            SpeedrunSettingsFileError::SplitsReadLock(lock) => fmt::Display::fmt(lock, f),
            SpeedrunSettingsFileError::SplitsWriteLock(lock) => fmt::Display::fmt(lock, f),
            SpeedrunSettingsFileError::Other(msg) => writeln!(f, "{msg}"),
        }
    }
}

impl<'a> fmt::Display for RunFileError<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RunFileError::UserInput(msg) => writeln!(f, "{msg}"),
            RunFileError::Save(se) => fmt::Display::fmt(se, f),
            RunFileError::Parse(ce) => fmt::Display::fmt(ce, f),
            RunFileError::IO(msg) => fmt::Display::fmt(msg, f),
            RunFileError::VarError(ve) => fmt::Display::fmt(ve, f),
            RunFileError::Dialog(de) => fmt::Display::fmt(de, f),
            RunFileError::TimerReadLock(lock) => fmt::Display::fmt(lock, f),
            RunFileError::TimerWriteLock(lock) => fmt::Display::fmt(lock, f),
            RunFileError::SplitsReadLock(lock) => fmt::Display::fmt(lock, f),
            RunFileError::SplitsWriteLock(lock) => fmt::Display::fmt(lock, f),
            RunFileError::Other(msg) => writeln!(f, "{msg}"),
        }
    }
}

impl<'a> fmt::Display for Error<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::UserInput(msg) => writeln!(f, "{msg}"),
            Error::VarError(ve) => fmt::Display::fmt(ve, f),
            Error::Dialog(de) => fmt::Display::fmt(de, f),
            Error::IO(ioe) => fmt::Display::fmt(ioe, f),
            Error::TimerReadLock(lock) => fmt::Display::fmt(lock, f),
            Error::TimerWriteLock(lock) => fmt::Display::fmt(lock, f),
            Error::SplitsReadLock(lock) => fmt::Display::fmt(lock, f),
            Error::SplitsWriteLock(lock) => fmt::Display::fmt(lock, f),
            Error::Other(msg) => writeln!(f, "{msg}"),
        }
    }
}

impl<'a> UserKeybinding<'_> {
    /// Represents keybindings provided by the user
    pub fn new(
        split_key: Option<&'a str>,
        reset_key: Option<&'a str>,
        pause_key: Option<&'a str>,
        unpause_key: Option<&'a str>,
        comparison_key: Option<&'a str>,
    ) -> UserKeybinding<'a> {
        UserKeybinding {
            split_key,
            reset_key,
            pause_key,
            unpause_key,
            comparison_key,
        }
    }
}

// Note: assumes linux environment
// Livesplit has an official release for windows so not it's not worth
// supporting windows
// TODO support macos
//
// Note: can't use $HOME as is
/// Returns "$HOME/.config/.speedrun_splits" expanded
fn default_config_path<'a>() -> Result<String, SSConfigurationFileError<'a>> {
    //let home = match std::env::var("HOME") {
    // NOTE: using std::env::var(HOME) with sudo gives root which is bad.
    // `su <sudoer USER>` works with desired user but you also give root access
    // which is also bad.
    // You need to not use sudo or login as user but still use /home/user/ as
    // base path.
    // NOTE search for granting default user the desired home without sudo
    // setpriv seems too broad when looking at capabilites and unrelated
    // do I have privileges to kb as myuser though?
    // NOTE login in as user does not grand enough privilege for kb
    // NOTE adding user to input group does give enough privileges
    // NOTE adding read permission to eventX (corresponding to kb plugged in)
    // does not allow reading
    // NOTE adding read and write permissions to eventX (corresponding to kb plugged in)
    // allows polling... (666)
    // NOTE plugging in then out kb resets permission
    // NOTE I could identify my device with:
    // udevadm info --attribute-walk --name /dev/input/event20 | grep "Gaming Keyboard G213"
    // ATTRS{name}=="Logitech Gaming Keyboard G213"
    // ATTRS{product}=="Gaming Keyboard G213"
    // NOTE changing group owner of /dev/input/eventX to $USER works.
    // However it does not persists as is. I can still use the keyboard for
    // everything though
    // NOTE on disconnect of the usb device, you would have to restart the app
    // TODO try udev rule to chmod the keyboard to <USER>
    // what device corresponds to my kb (follow symlink with a)
    // ls -la /dev/input/by-id/
    // lrwxrwxrwx 1 root root  10 May 29 19:57 usb-Logitech_Gaming_Keyboard_G213_0D8836713737-event-kbd -> ../event20
    // what attribute can I use?
    // udevadm info --name /dev/input/event20
    // SUBSYSTEM=input
    // ID_SERIAL=Logitech_Gaming_Keyboard_G213_0D8836713737
    // try better
    // sudo udevadm info --attribute-walk --name /dev/input/event8 | grep serial
    // ATTRS{serial}=="0D8836713737"
    // ATTRS{serial}=="0000:00:14.0"
    // sudo udevadm info --attribute-walk --name /dev/input/event8 | grep name
    // ATTRS{name}=="Logitech Gaming Keyboard G213 Keyboard"
    let home = std::env::var("HOME")?;
    Ok(format!("{home}/.config/.speedrun_splits"))
}

/// Returns "$HOME/.speedrun_splits" expanded
pub fn default_data_folder<'a>() -> Result<String, Error<'a>> {
    // Note: if executed with sudo, home will default to /root, which is usually not desired
    let home = std::env::var("HOME")?;
    Ok(format!("{home}/.speedrun_splits"))
}

/// Returns "$HOME/.speedrun_splits/logs.txt"
pub fn default_log_file_path<'a>() -> Result<String, Error<'a>> {
    let home = std::env::var("HOME")?;
    Ok(format!("{home}/.speedrun_splits/logs.txt"))
}

impl SpeedrunSettings {
    /// Get file name of speedrun settings
    fn get_file_name(&self) -> String {
        format!("{}_{}.txt", self.game_name, self.category_name)
    }

    /// Get file name of associated run file
    fn get_run_file_name(&self) -> String {
        format!("{}_{}.lss", self.game_name, self.category_name)
    }

    /// Return the name of the game for this speedrun
    pub fn get_game_name(&self) -> String {
        self.game_name.clone()
    }

    /// Return the name of the game category for this speedrun
    pub fn get_category_name(&self) -> String {
        self.category_name.clone()
    }

    /// Return names of splits for this speedrun
    pub fn get_split_names(&self) -> Vec<String> {
        self.split_names.clone()
    }

    /// Get split key from this speedrun settings
    pub fn get_split_key(&self) -> KeyCode {
        self.keybindings.split_key
    }

    /// Get reset key from this speedrun settings
    pub fn get_reset_key(&self) -> KeyCode {
        self.keybindings.reset_key
    }

    /// Get pause key from this speedrun settings
    pub fn get_pause_key(&self) -> KeyCode {
        self.keybindings.pause_key
    }

    /// Get unpause key from this speedrun settings
    pub fn get_unpause_key(&self) -> KeyCode {
        self.keybindings.unpause_key
    }

    /// Get comparison key from this speedrun settings
    pub fn get_comparison_key(&self) -> KeyCode {
        self.keybindings.comparison_key
    }
}

/// Parse configuration file at default path and ask user if not present
pub fn parse_configuration<'a>() -> Result<SpeedrunSplitsConfiguration, SSConfigurationFileError<'a>>
{
    let default_config_path = default_config_path()?;
    let default_data_folder = default_data_folder()?;
    if !Path::new(default_config_path.as_str()).exists() {
        let choice = dialog::Question::new(format!("No configuration file was found at \"{default_config_path}\". Create configuration file?").as_str())
    .title("Create configuration file?")
    .show()?;
        if choice == dialog::Choice::Yes {
            let config = SpeedrunSplitsConfiguration::new()?;
            match save_config_to_file(&config) {
                Ok(()) => return Ok(config),
                Err(e) => return Err(e),
            };
        }
    }

    if !Path::new(default_data_folder.as_str()).exists() {
        let choice = dialog::Question::new(
            format!("No data folder was found at \"{default_data_folder}\". Create data folder?")
                .as_str(),
        )
        .title("Create data folder?")
        .show()?;
        if choice == dialog::Choice::Yes {
            fs::create_dir(default_data_folder.as_str())?;
        }
    }

    trace!("Parsing configuration file");
    let mut file = File::open(default_config_path.as_str())?;
    let mut config = String::new();
    file.read_to_string(&mut config)?;
    let config: SpeedrunSplitsConfiguration = toml::from_str(config.as_str())?;
    Ok(config)
}

/// Load speedrun from file and return true if newly created
///
/// Use user provided `game_name` and `category_name` if present. Otherwise,
/// fall back to default speedrun
pub fn load_speedrun_settings<'a>(
    configuration: &'a SpeedrunSplitsConfiguration,
    game_name: Option<&str>,
    category_name: Option<&str>,
    split_names: Option<&str>,
    user_keybinding: UserKeybinding,
) -> (
    Result<SpeedrunSettings, SpeedrunSettingsFileError<'a>>,
    bool,
) {
    if let Some(game_name) = game_name {
        if let Some(category_name) = category_name {
            let settings = SpeedrunSettings {
                game_name: game_name.to_string(),
                category_name: category_name.to_string(),
                split_names: vec![],
                keybindings: Keybinding {
                    split_key: KeyCode::Numpad1,
                    reset_key: KeyCode::Numpad3,
                    pause_key: KeyCode::Numpad5,
                    unpause_key: KeyCode::Numpad7,
                    comparison_key: KeyCode::Numpad9,
                },
            };
            return (
                find_speedrun_by_name(settings.get_file_name(), configuration),
                false,
            );
        }
    }

    if configuration.use_default_speedrun {
        info!("Loading default speedrun");
        match configuration.default_speedrun_name.clone() {
            Some(n) => (find_speedrun_by_name(n, configuration), false),
            None => {
                warn!("No default speedrun name was set. Have you set a default_speedrun_name entry in your configuration file?");
                (
                    ask_speedrun_settings_to_user(
                        game_name,
                        category_name,
                        split_names,
                        user_keybinding,
                    ),
                    true,
                )
            }
        }
    } else {
        (
            ask_speedrun_settings_to_user(game_name, category_name, split_names, user_keybinding),
            true,
        )
    }
}

/// Search data folder from `configuration` for speedrun with provided `name`
fn find_speedrun_by_name(
    name: String,
    configuration: &SpeedrunSplitsConfiguration,
) -> Result<SpeedrunSettings, SpeedrunSettingsFileError> {
    let data_folder_path = configuration.data_folder_path.as_str();
    if name.is_empty() {
        return Err(SpeedrunSettingsFileError::UserInput(
            "Speedrun name cannot be empty.".to_string(),
        ));
    }
    debug!("parsing {data_folder_path}");
    for entry in WalkDir::new(data_folder_path) {
        let e = match entry {
            Ok(e) => e,
            Err(e) => {
                warn!("Skipping entry that could not be parsed");
                debug!("Skipped entry: {e}");
                continue;
            }
        };
        let ss_file = e.path().display().to_string();
        let file_name = e.path().file_name().ok_or("could not get file name")?;
        let ss_entry = file_name.to_str().ok_or("Error converting file name")?;
        debug!("{ss_entry}");
        if ss_entry == name {
            info!("Found speedrun");
            trace!("Parsing speedrun settings file");
            let mut file = File::open(ss_file.as_str())?;
            let mut ss_settings = String::new();
            file.read_to_string(&mut ss_settings)?;
            let ss: SpeedrunSettings = toml::from_str(&ss_settings)?;
            return Ok(ss);
        }
    }
    Err(SpeedrunSettingsFileError::UserInput(format!(
        "Did not find speedrun with name {}",
        name
    )))
}

/// Get user output. Exit program if user exits dialog
fn get_user_output<'a>(input: &mut Input) -> Result<String, Error<'a>> {
    match input.show() {
        Ok(i) => match i {
            Some(i) => Ok(i),
            // NOTE: either cancel option was chosen or dialog was closed
            None => {
                // TODO return error with type UserCancel and exit elsewhere
                info!("User stopped filling speedrun settings.");
                std::process::exit(0);
            }
        },
        Err(e) => {
            return Err(Error::Dialog(e));
        }
    }
}

/// Ask user for speedrun settings
fn ask_speedrun_settings_to_user<'a>(
    game_name: Option<&str>,
    category_name: Option<&str>,
    split_names: Option<&str>,
    keybinding: UserKeybinding,
) -> Result<SpeedrunSettings, SpeedrunSettingsFileError<'a>> {
    // Continuously ask for non-empty game name
    let mut game_name: String = game_name.unwrap_or_default().to_string();
    while game_name.is_empty() {
        let gn = get_user_output(
            Input::new("Please enter the game name (cannot be empty):").title("Enter game name"),
        )?;
        game_name = gn;
    }

    let mut category_name: String = category_name.unwrap_or_default().to_string();
    while category_name.is_empty() {
        let cn = get_user_output(
            Input::new("Please enter the category name (cannot be empty):")
                .title("Enter category name"),
        )?;
        category_name = cn;
    }

    let mut split_names: Vec<String> = get_splits(split_names.unwrap_or_default().to_string());
    while split_names.is_empty() {
        let sn = get_user_output(dialog::Input::new(
            "Please provide at least one split, each separated with '|' (for instance 'split1|split 2|split 3'):",
        ).title("Enter split names"))?;
        split_names = get_splits(sn);
    }

    loop {
        // NOTE: KeyCode does not implement display but Debug matches serialized string
        let split_key = match keybinding.split_key {
            Some(k) => parse_key(k.to_string())?,
            None => ask_user_keybinding("start/split".to_string(), format!("{Numpad1:?}"))?,
        };
        let reset_key = match keybinding.reset_key {
            Some(k) => parse_key(k.to_string())?,
            None => ask_user_keybinding("reset".to_string(), format!("{Numpad3:?}"))?,
        };
        let pause_key = match keybinding.pause_key {
            Some(k) => parse_key(k.to_string())?,
            None => ask_user_keybinding("pause".to_string(), format!("{Numpad5:?}"))?,
        };
        let unpause_key = match keybinding.unpause_key {
            Some(k) => parse_key(k.to_string())?,
            None => ask_user_keybinding("unpause".to_string(), format!("{Numpad7:?}"))?,
        };
        let comparison_key = match keybinding.comparison_key {
            Some(k) => parse_key(k.to_string())?,
            None => ask_user_keybinding("comparison".to_string(), format!("{Numpad9:?}"))?,
        };

        let keys = vec![split_key, reset_key, pause_key, unpause_key];
        if keys.iter().all_unique() {
            let keybinding =
                Keybinding::new(split_key, reset_key, pause_key, unpause_key, comparison_key);
            return SpeedrunSettings::new(split_names, game_name, category_name, keybinding);
        } else {
            warn!("No two keybinds can be the same. Retrying...")
        }
    }
}

/// Ask user keybinding for `key` while displaying `help`
fn ask_user_keybinding<'a>(
    key_name: String,
    example_keybind: String,
) -> Result<KeyCode, Error<'a>> {
    let description = format!("Please provide the {key_name} key (example: \"{example_keybind}\", all possible values https://github.com/obv-mikhail/InputBot/blob/develop/src/public.rs):");
    let title = format!("Provide {key_name} key");
    let k = get_user_output(Input::new(description).title(title))?;
    let k = parse_key(k)?;
    Ok(k)
}

/// Get splits from `raw_splits`
fn get_splits(raw_splits: String) -> Vec<String> {
    raw_splits
        .split('|')
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Enabled default speedrun present in `settings` by updating `configuration`
pub fn update_configuration_with_default_speedrun(
    configuration: SpeedrunSplitsConfiguration,
    settings: &SpeedrunSettings,
) -> Result<(), SSConfigurationFileError> {
    let game_name = settings.game_name.as_str();
    let category_name = settings.category_name.as_str();
    let choice = dialog::Question::new(format!("Make \"{game_name}: {category_name}\" default?"))
        .title("Make speedrun default?")
        .show()?;
    if choice == dialog::Choice::Yes {
        let mut config = configuration;
        config.use_default_speedrun = true;
        config.default_speedrun_name = Some(settings.get_file_name());
        save_config_to_file(&config)
    } else {
        Ok(())
    }
}

/// Save `configuration` to file
fn save_config_to_file<'a>(
    configuration: &SpeedrunSplitsConfiguration,
) -> Result<(), SSConfigurationFileError<'a>> {
    let mut file = File::create(default_config_path()?.as_str())?;
    let config_content = toml::to_string(&configuration)?;
    file.write_all(config_content.as_bytes())?;
    info!("Configuration file created");
    Ok(())
}

/// Save speedrun `settings` to file
pub fn save_speedrun_settings_to_file(
    settings: &SpeedrunSettings,
) -> Result<(), SpeedrunSettingsFileError> {
    let default_data_folder = default_data_folder()?;
    let file_path = format!("{}/{}", default_data_folder, settings.get_file_name());
    let mut file = File::create(file_path)?;
    let settings_content = toml::to_string(&settings)?;
    file.write_all(settings_content.as_bytes())?;

    info!("Speedrun settings file saved");
    Ok(())
}

/// Save `run` to file that corresponds to speedrun `settings`
pub fn save_run_to_file<'a>(
    run: &Run,
    settings: &SpeedrunSettings,
) -> Result<(), RunFileError<'a>> {
    let default_data_folder = default_data_folder()?;
    let file_path = format!("{default_data_folder}/{}", settings.get_run_file_name());
    let file = File::create(file_path)?;
    let writer = BufWriter::new(file);
    livesplit::save_run(run, writer)?;
    Ok(())
}

/// Parse run from data folder present in `settings`
pub fn parse_run_from_file(settings: &SpeedrunSettings) -> Result<Run, RunFileError> {
    // Load the file.
    let default_data_folder = default_data_folder()?;
    let file_path = PathBuf::from(format!(
        "{default_data_folder}/{}",
        settings.get_run_file_name()
    ));
    let file = File::open(&file_path)?;
    let file = BufReader::new(file);

    // We want to load additional files from the file system, like segment icons.
    let load_files = true;

    // Actually parse the file.
    let parsed = composite::parse(file, Some(file_path), load_files)?;

    // Print out the detected file format.
    info!("Splits File Format: {}", parsed.kind);

    // Get out the Run object.
    let run = parsed.run;
    Ok(run)
}
