//! Persist user files for the speedrun_splits application
//!
//! * run file (.lss)
//! * settings (.txt) associated with speedrun
//! * general configuration (`$HOME/.config/.speedrun_splits`)
//! * log file

use crate::parse_key;
use crate::Error as lError;
use dialog::{DialogBox, Input};
use inputbot::KeybdKey;
use inputbot::KeybdKey::{Numpad1Key, Numpad3Key, Numpad7Key, Numpad9Key};
use itertools::Itertools;
use livesplit_core::run::parser::composite;
use livesplit_core::run::saver::livesplit;
use livesplit_core::Run;
use log::*;
use serde::{Deserialize, Serialize};
use std::env::VarError;
use std::fs::File;
use std::io::BufReader;
use std::io::BufWriter;
use std::io::{Read, Write};
use std::path::Path;
use std::path::PathBuf;
use std::{fmt, fs};
use walkdir::WalkDir;

#[derive(Serialize, Deserialize)]
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
    pub split_names: Vec<String>,
    pub game_name: String,
    pub category_name: String,
    pub split_key: String,
    pub reset_key: String,
    pub pause_key: String,
    pub unpause_key: String,
}

/// Keybinding provided by the user
pub struct UserKeybinding<'a> {
    split_key: Option<&'a str>,
    reset_key: Option<&'a str>,
    pause_key: Option<&'a str>,
    unpause_key: Option<&'a str>,
}

#[derive(Debug, Eq, PartialEq, Hash)]
/// Errors while persisting necessary files for the speedrun_splits application
pub enum Error {
    User(String),
    Dialog(String),
    IO(String),
    Other(String),
}

/// Errors while using the configuration file of the speedrun_splits application
pub enum SpeedrunSplitsConfigurationFileError {
    User(String),
    Dialog(String),
    IO(String),
    DataFolder(String),
    Conversion(String),
    Other(String),
}

/// Errors while using the settings file of a speedrun
pub enum SpeedrunSettingsFileError {
    User(String),
    Dialog(String),
    Conversion(String),
    IO(String),
    Other(String),
}

/// Errors while using the run file of a speedrun
pub enum RunFileError {
    Save(String),
    Parse(String),
    User(String),
    IO(String),
}

impl SpeedrunSettings {
    fn new(
        split_names: Vec<String>,
        game_name: String,
        category_name: String,
        split_key: KeybdKey,
        reset_key: KeybdKey,
        pause_key: KeybdKey,
        unpause_key: KeybdKey,
    ) -> Result<SpeedrunSettings, SpeedrunSettingsFileError> {
        let keys = vec![split_key, reset_key, pause_key, unpause_key];
        if !keys.iter().all_unique() {
            return Err(SpeedrunSettingsFileError::User(
                "All keys need to be bound to a different key".to_string(),
            ));
        }

        let keys = vec![split_key, reset_key];
        if keys.iter().all_unique() {}

        Ok(SpeedrunSettings {
            split_names,
            game_name,
            category_name,
            split_key: get_key_name(split_key),
            reset_key: get_key_name(reset_key),
            pause_key: get_key_name(pause_key),
            unpause_key: get_key_name(unpause_key),
        })
    }
}

impl SpeedrunSplitsConfiguration {
    fn new() -> Result<SpeedrunSplitsConfiguration, SpeedrunSplitsConfigurationFileError> {
        Ok(SpeedrunSplitsConfiguration {
            data_folder_path: default_data_folder()?,
            default_speedrun_name: None,
            use_default_speedrun: true,
        })
    }
}

impl fmt::Display for SpeedrunSplitsConfigurationFileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SpeedrunSplitsConfigurationFileError::User(msg) => writeln!(f, "{msg}"),
            SpeedrunSplitsConfigurationFileError::Dialog(msg) => writeln!(f, "{msg}"),
            SpeedrunSplitsConfigurationFileError::IO(msg) => writeln!(f, "{msg}"),
            SpeedrunSplitsConfigurationFileError::DataFolder(msg) => writeln!(f, "{msg}"),
            SpeedrunSplitsConfigurationFileError::Conversion(msg) => writeln!(f, "{msg}"),
            SpeedrunSplitsConfigurationFileError::Other(msg) => writeln!(f, "{msg}"),
        }
    }
}

impl From<VarError> for SpeedrunSplitsConfigurationFileError {
    fn from(e: VarError) -> Self {
        SpeedrunSplitsConfigurationFileError::User(e.to_string())
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::IO(e.to_string())
    }
}

impl From<dialog::Error> for SpeedrunSplitsConfigurationFileError {
    fn from(e: dialog::Error) -> Self {
        SpeedrunSplitsConfigurationFileError::Dialog(e.to_string())
    }
}

impl From<std::io::Error> for RunFileError {
    fn from(e: std::io::Error) -> Self {
        RunFileError::IO(e.to_string())
    }
}

impl From<VarError> for Error {
    fn from(e: VarError) -> Self {
        Error::User(e.to_string())
    }
}

impl From<Error> for RunFileError {
    fn from(e: Error) -> Self {
        match e {
            Error::User(msg) => RunFileError::User(msg),
            _ => panic!(""),
        }
    }
}

impl From<livesplit_core::run::parser::composite::Error> for RunFileError {
    fn from(e: livesplit_core::run::parser::composite::Error) -> Self {
        RunFileError::Parse(e.to_string())
    }
}

impl From<livesplit_core::run::saver::livesplit::Error> for RunFileError {
    fn from(e: livesplit_core::run::saver::livesplit::Error) -> Self {
        RunFileError::Save(e.to_string())
    }
}

impl From<std::io::Error> for SpeedrunSplitsConfigurationFileError {
    fn from(e: std::io::Error) -> Self {
        SpeedrunSplitsConfigurationFileError::IO(e.to_string())
    }
}

impl From<lError> for Error {
    fn from(e: lError) -> Self {
        match e {
            lError::UserInput(msg) => Error::User(msg),
            lError::UI(msg) => Error::Other(format!("UI: {msg}")),
            lError::Timer(msg) => Error::Other(format!("Timer: {msg}")),
            lError::Other(msg) => Error::Other(msg),
        }
    }
}

impl From<Error> for SpeedrunSplitsConfigurationFileError {
    fn from(e: Error) -> Self {
        match e {
            Error::User(msg) => SpeedrunSplitsConfigurationFileError::User(msg),
            Error::Dialog(msg) => SpeedrunSplitsConfigurationFileError::Dialog(msg),
            Error::IO(msg) => SpeedrunSplitsConfigurationFileError::IO(msg),
            Error::Other(msg) => SpeedrunSplitsConfigurationFileError::Other(msg),
        }
    }
}

impl From<toml::de::Error> for SpeedrunSettingsFileError {
    fn from(e: toml::de::Error) -> Self {
        SpeedrunSettingsFileError::Conversion(e.to_string())
    }
}

impl From<toml::ser::Error> for SpeedrunSplitsConfigurationFileError {
    fn from(e: toml::ser::Error) -> Self {
        SpeedrunSplitsConfigurationFileError::Conversion(e.to_string())
    }
}

impl From<toml::ser::Error> for SpeedrunSettingsFileError {
    fn from(e: toml::ser::Error) -> Self {
        SpeedrunSettingsFileError::Conversion(e.to_string())
    }
}

impl From<std::io::Error> for SpeedrunSettingsFileError {
    fn from(e: std::io::Error) -> Self {
        SpeedrunSettingsFileError::IO(e.to_string())
    }
}

impl From<&str> for SpeedrunSettingsFileError {
    fn from(e: &str) -> Self {
        SpeedrunSettingsFileError::Conversion(e.to_string())
    }
}

impl From<walkdir::Error> for SpeedrunSplitsConfigurationFileError {
    fn from(e: walkdir::Error) -> Self {
        SpeedrunSplitsConfigurationFileError::DataFolder(e.to_string())
    }
}

impl From<SpeedrunSplitsConfigurationFileError> for std::fmt::Error {
    fn from(e: SpeedrunSplitsConfigurationFileError) -> Self {
        // log error
        error!("{e}");
        std::fmt::Error
    }
}

impl From<Error> for std::fmt::Error {
    fn from(e: Error) -> Self {
        // log error
        error!("{e}");
        std::fmt::Error
    }
}

impl From<lError> for SpeedrunSettingsFileError {
    fn from(e: lError) -> Self {
        match e {
            lError::UserInput(msg) => SpeedrunSettingsFileError::User(msg),
            lError::UI(msg) => SpeedrunSettingsFileError::Other(format!("UI: {msg}")),
            lError::Timer(msg) => SpeedrunSettingsFileError::Other(format!("Timer: {msg}")),
            lError::Other(msg) => SpeedrunSettingsFileError::Other(msg),
        }
    }
}

impl From<Error> for SpeedrunSettingsFileError {
    fn from(e: Error) -> Self {
        match e {
            Error::Dialog(msg) => SpeedrunSettingsFileError::Dialog(msg),
            Error::User(msg) => SpeedrunSettingsFileError::User(msg),
            Error::IO(msg) => SpeedrunSettingsFileError::IO(msg),
            Error::Other(msg) => SpeedrunSettingsFileError::Other(msg),
        }
    }
}

impl From<toml::de::Error> for SpeedrunSplitsConfigurationFileError {
    fn from(e: toml::de::Error) -> Self {
        SpeedrunSplitsConfigurationFileError::Conversion(e.to_string())
    }
}

impl fmt::Display for SpeedrunSettingsFileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match self {
            SpeedrunSettingsFileError::User(msg) => msg.to_owned(),
            SpeedrunSettingsFileError::Dialog(msg) => msg.to_owned(),
            SpeedrunSettingsFileError::Conversion(msg) => msg.to_owned(),
            SpeedrunSettingsFileError::IO(msg) => msg.to_owned(),
            SpeedrunSettingsFileError::Other(msg) => msg.to_owned(),
        };
        writeln!(f, "{msg}")
    }
}

impl fmt::Display for RunFileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match self {
            RunFileError::Save(msg) => msg.to_owned(),
            RunFileError::Parse(msg) => msg.to_owned(),
            RunFileError::User(msg) => msg.to_owned(),
            RunFileError::IO(msg) => msg.to_owned(),
        };
        write!(f, "{msg}")
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg: String = match self {
            Error::User(msg) => msg.to_owned(),
            Error::Dialog(msg) => msg.to_owned(),
            Error::IO(msg) => msg.to_owned(),
            Error::Other(msg) => msg.to_owned(),
        };
        write!(f, "{msg}")
    }
}

impl<'a> UserKeybinding<'_> {
    pub fn new(
        split_key: Option<&'a str>,
        reset_key: Option<&'a str>,
        pause_key: Option<&'a str>,
        unpause_key: Option<&'a str>,
    ) -> UserKeybinding<'a> {
        UserKeybinding {
            split_key,
            reset_key,
            pause_key,
            unpause_key,
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
fn default_config_path() -> Result<String, SpeedrunSplitsConfigurationFileError> {
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
pub fn default_data_folder() -> Result<String, Error> {
    // Note: if executed with sudo, home will default to /root, which is usually not desired
    let home = std::env::var("HOME")?;
    Ok(format!("{home}/.speedrun_splits"))
}

/// Returns "$HOME/.speedrun_splits/logs.txt"
pub fn default_log_file_path() -> Result<String, Error> {
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
}

/// Parse configuration file at default path and ask user if not present
pub fn parse_configuration(
) -> Result<SpeedrunSplitsConfiguration, SpeedrunSplitsConfigurationFileError> {
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
pub fn load_speedrun_settings(
    configuration: &SpeedrunSplitsConfiguration,
    game_name: Option<&str>,
    category_name: Option<&str>,
    split_names: Option<&str>,
    user_keybinding: UserKeybinding,
) -> (Result<SpeedrunSettings, SpeedrunSettingsFileError>, bool) {
    if let Some(game_name) = game_name {
        if let Some(category_name) = category_name {
            let settings = SpeedrunSettings {
                game_name: game_name.to_string(),
                category_name: category_name.to_string(),
                split_names: vec![],
                split_key: String::new(),
                reset_key: String::new(),
                pause_key: String::new(),
                unpause_key: String::new(),
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
        return Err(SpeedrunSettingsFileError::User(
            "Speedrun name cannot be empty.".to_string(),
        ));
    }
    debug!("parsing {data_folder_path}");
    for entry in WalkDir::new(data_folder_path) {
        let e = match entry {
            Ok(e) => e,
            Err(e) => {
                warn!("Skipping entry that could not be parsed");
                debug!("Skipped entry error: {e}");
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
    Err(SpeedrunSettingsFileError::User(format!(
        "Did not find speedrun with name {}",
        name
    )))
}

/// Get user output. Exit program if user exits dialog
fn get_user_output(input: &mut Input) -> Result<String, Error> {
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
            return Err(Error::Dialog(format!("{e}")));
        }
    }
}

/// Ask user for speedrun settings
fn ask_speedrun_settings_to_user(
    game_name: Option<&str>,
    category_name: Option<&str>,
    split_names: Option<&str>,
    keybinding: UserKeybinding,
) -> Result<SpeedrunSettings, SpeedrunSettingsFileError> {
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
    //                match get_user_output(Input::new(
    //            "Please provide the reset key (example: \"Numpad3Key\", all possible values https://github.com/obv-mikhail/InputBot/blob/develop/src/public.rs):",
    //        )
    //        .title("Provide reset key")) {
    // ask user split and reset key. Prefer cli args if provided. When invalid
    // argument are provided, ask user.
    // Split and reset key cannot be the same
    loop {
        let split_key = match keybinding.split_key {
            Some(k) => parse_key(k.to_string())?,
            None => ask_user_keybinding("start/split", format!("{:?}", Numpad1Key).as_str())?,
        };
        let reset_key = match keybinding.reset_key {
            Some(k) => parse_key(k.to_string())?,
            None => ask_user_keybinding("reset", format!("{:?}", Numpad3Key).as_str())?,
        };
        let pause_key = match keybinding.pause_key {
            Some(k) => parse_key(k.to_string())?,
            None => ask_user_keybinding("pause", format!("{:?}", Numpad7Key).as_str())?,
        };
        let unpause_key = match keybinding.unpause_key {
            Some(k) => parse_key(k.to_string())?,
            None => ask_user_keybinding("unpause", format!("{:?}", Numpad9Key).as_str())?,
        };

        let keys = vec![split_key, reset_key, pause_key, unpause_key];

        if keys.iter().all_unique() {
            return SpeedrunSettings::new(
                split_names,
                game_name,
                category_name,
                split_key,
                reset_key,
                pause_key,
                unpause_key,
            );
        } else {
            warn!("No two keybinds can be the same. Retrying...")
        }
    }
}

/// Ask user keybinding for `key` while displaying `help`
fn ask_user_keybinding(key_name: &str, example_keybind: &str) -> Result<KeybdKey, Error> {
    let k = get_user_output(Input::new(format!(
"Please provide the {key_name} key (example: \"{example_keybind}\", all possible values https://github.com/obv-mikhail/InputBot/blob/develop/src/public.rs):",
            )).title(format!("Provide {key_name} key")))?;

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

/// Get keyname from enum
fn get_key_name(k: KeybdKey) -> String {
    format!("{:?}", k)
}

/// Enabled default speedrun present in `settings` by updating `configuration`
pub fn update_configuration_with_default_speedrun(
    configuration: SpeedrunSplitsConfiguration,
    settings: &SpeedrunSettings,
) -> Result<(), SpeedrunSplitsConfigurationFileError> {
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
fn save_config_to_file(
    configuration: &SpeedrunSplitsConfiguration,
) -> Result<(), SpeedrunSplitsConfigurationFileError> {
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
pub fn save_run_to_file(run: &Run, settings: &SpeedrunSettings) -> Result<(), RunFileError> {
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
