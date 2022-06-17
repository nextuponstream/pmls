//! Interact with user to parse and persist necessary configurations files:
//!
//! * run file (.lss)
//! * settings (.txt) associated with speedrun
//! * general configuration (`$HOME/.config/.pmls`)
//! * log file
use crate::Keybinding;
use clap::Values;
use dialog::{DialogBox, Input};
use itertools::Itertools;
use livesplit_core::hotkey::KeyCode;
use livesplit_core::hotkey::KeyCode::{Numpad1, Numpad3, Numpad5, Numpad7, Numpad9};
use livesplit_core::run::{parser::composite, saver::livesplit};
use livesplit_core::Run;
use log::{debug, error, info, trace, warn};
use serde::{Deserialize, Serialize};
use std::env::VarError;
use std::fmt::Debug;
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::{fmt, fs};
use walkdir::WalkDir;

#[derive(Serialize, Deserialize, Clone)]
/// Configuration file of the poor man's livesplit application
pub struct PMLSConfiguration {
    data_folder_path: String,
    /// open default speedrun when launching application with no arguments
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
#[derive(Copy, Clone)]
pub struct UserKeybinding<'a> {
    split_key: Option<&'a str>,
    reset_key: Option<&'a str>,
    pause_key: Option<&'a str>,
    unpause_key: Option<&'a str>,
    comparison_key: Option<&'a str>,
}

#[derive(Debug)]
/// Errors while persisting necessary files for the pmls application
pub enum FileError {
    /// Input from user is invalid
    UserInput(String),
    /// User did not provide asked input
    UserCancel(),
    /// Error with the user environment variables
    VarError(VarError),
    /// User cannot interact with dialog box
    Dialog(dialog::Error),
    /// Error with filesystem
    IO(std::io::Error),
    /// Unrecoverable error such as division by zero
    Other(String),
}

/// Errors while using the configuration file of the pmls application
pub enum ConfigurationFileError {
    /// Input from user is invalid
    UserInput(String),
    /// User did not accept creating necessary configuration file
    UserCancel(),
    /// Error with the user environment variables
    VarError(VarError),
    /// User cannot interact with dialog box
    Dialog(dialog::Error),
    /// Error with filesystem
    IO(std::io::Error),
    /// Error while parsing user's pmls data folder
    DataFolder(walkdir::Error),
    /// Serialization to toml format error
    Serialize(toml::ser::Error),
    /// Deserialization from toml format error
    Deserialize(toml::de::Error),
    /// Unrecoverable error such as division by zero
    Other(String),
}

/// Errors while using the settings file of a speedrun
// NOTE: cannot print error directly because contained errors
//       (example: dialog::Error) do not implement Copy or Clone
// TODO implement Clone or Copy
#[derive(Debug)]
pub enum SpeedrunSettingsFileError {
    /// Input from user is invalid
    UserInput(String),
    /// User did not provide asked input
    UserCancel(),
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
    /// Unrecoverable error such as division by zero
    Other(String),
    /// Missing settings file
    Missing(String),
}

/// Errors while using the run file of a speedrun
pub enum RunFileError {
    /// Input from user is invalid
    UserInput(String),
    /// User did not provide asked input
    UserCancel(),
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
    /// Unrecoverable error such as division by zero
    Other(String),
}

impl<'a> SpeedrunSettings {
    fn new(
        split_names: Vec<String>,
        game_name: String,
        category_name: String,
        keybindings: Keybinding,
    ) -> Result<SpeedrunSettings, SpeedrunSettingsFileError> {
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

impl<'a> PMLSConfiguration {
    fn new() -> Result<PMLSConfiguration, ConfigurationFileError> {
        Ok(PMLSConfiguration {
            data_folder_path: default_data_folder()?,
            default_speedrun_name: None,
            use_default_speedrun: true,
        })
    }
}

impl<'a> fmt::Display for ConfigurationFileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigurationFileError::UserInput(msg) | ConfigurationFileError::Other(msg) => {
                writeln!(f, "{msg}")
            }
            ConfigurationFileError::UserCancel() => {
                writeln!(f, "User cancelled creating general configuration file")
            }
            ConfigurationFileError::Dialog(de) => fmt::Display::fmt(de, f),
            ConfigurationFileError::IO(ioe) => fmt::Display::fmt(ioe, f),
            ConfigurationFileError::DataFolder(wde) => fmt::Display::fmt(wde, f),
            ConfigurationFileError::Serialize(se) => fmt::Display::fmt(se, f),
            ConfigurationFileError::Deserialize(de) => fmt::Display::fmt(de, f),
            ConfigurationFileError::VarError(v) => fmt::Display::fmt(v, f),
        }
    }
}

impl<'a> From<std::io::Error> for FileError {
    fn from(e: std::io::Error) -> Self {
        FileError::IO(e)
    }
}

impl<'a> From<VarError> for FileError {
    fn from(e: VarError) -> Self {
        FileError::VarError(e)
    }
}

impl<'a> From<VarError> for ConfigurationFileError {
    fn from(e: VarError) -> Self {
        ConfigurationFileError::VarError(e)
    }
}

impl<'a> From<dialog::Error> for ConfigurationFileError {
    fn from(e: dialog::Error) -> Self {
        ConfigurationFileError::Dialog(e)
    }
}

impl<'a> From<dialog::Error> for SpeedrunSettingsFileError {
    fn from(e: dialog::Error) -> Self {
        SpeedrunSettingsFileError::Dialog(e)
    }
}

impl<'a> From<std::io::Error> for RunFileError {
    fn from(e: std::io::Error) -> Self {
        RunFileError::IO(e)
    }
}

impl<'a> From<FileError> for RunFileError {
    fn from(e: FileError) -> Self {
        match e {
            FileError::UserInput(msg) => RunFileError::UserInput(msg),
            FileError::UserCancel() => RunFileError::UserCancel(),
            FileError::VarError(ve) => RunFileError::VarError(ve),
            FileError::Dialog(de) => RunFileError::Dialog(de),
            FileError::IO(io) => RunFileError::IO(io),
            FileError::Other(msg) => RunFileError::Other(msg),
        }
    }
}

impl<'a> From<livesplit_core::run::parser::composite::Error> for RunFileError {
    fn from(e: livesplit_core::run::parser::composite::Error) -> Self {
        RunFileError::Parse(e)
    }
}

impl<'a> From<livesplit_core::run::saver::livesplit::Error> for RunFileError {
    fn from(e: livesplit_core::run::saver::livesplit::Error) -> Self {
        RunFileError::Save(e)
    }
}

impl<'a> From<std::io::Error> for ConfigurationFileError {
    fn from(io: std::io::Error) -> Self {
        ConfigurationFileError::IO(io)
    }
}

impl<'a> From<FileError> for ConfigurationFileError {
    fn from(e: FileError) -> Self {
        match e {
            FileError::UserInput(msg) => ConfigurationFileError::UserInput(msg),
            FileError::UserCancel() => ConfigurationFileError::UserCancel(),
            FileError::Dialog(de) => ConfigurationFileError::Dialog(de),
            FileError::IO(io) => ConfigurationFileError::IO(io),
            FileError::VarError(ve) => ConfigurationFileError::VarError(ve),
            FileError::Other(msg) => ConfigurationFileError::Other(msg),
        }
    }
}

impl<'a> From<toml::de::Error> for SpeedrunSettingsFileError {
    fn from(e: toml::de::Error) -> Self {
        SpeedrunSettingsFileError::Deserialize(e)
    }
}

impl<'a> From<toml::ser::Error> for ConfigurationFileError {
    fn from(e: toml::ser::Error) -> Self {
        ConfigurationFileError::Serialize(e)
    }
}

impl<'a> From<toml::ser::Error> for SpeedrunSettingsFileError {
    fn from(e: toml::ser::Error) -> Self {
        SpeedrunSettingsFileError::Serialize(e)
    }
}

impl<'a> From<std::io::Error> for SpeedrunSettingsFileError {
    fn from(e: std::io::Error) -> Self {
        SpeedrunSettingsFileError::IO(e)
    }
}

impl<'a> From<&str> for SpeedrunSettingsFileError {
    fn from(e: &str) -> Self {
        SpeedrunSettingsFileError::OSStringConversion(e.to_string())
    }
}

impl<'a> From<walkdir::Error> for ConfigurationFileError {
    fn from(e: walkdir::Error) -> Self {
        ConfigurationFileError::DataFolder(e)
    }
}

impl<'a> From<ConfigurationFileError> for std::fmt::Error {
    fn from(e: ConfigurationFileError) -> Self {
        // log error before losing information
        error!("{e}");
        std::fmt::Error
    }
}

impl<'a> From<FileError> for std::fmt::Error {
    fn from(e: FileError) -> Self {
        // log error before losing information
        error!("{e}");
        std::fmt::Error
    }
}

impl<'a> From<FileError> for SpeedrunSettingsFileError {
    fn from(e: FileError) -> Self {
        match e {
            FileError::Dialog(de) => SpeedrunSettingsFileError::Dialog(de),
            FileError::UserInput(msg) => SpeedrunSettingsFileError::UserInput(msg),
            FileError::UserCancel() => SpeedrunSettingsFileError::UserCancel(),
            FileError::IO(io) => SpeedrunSettingsFileError::IO(io),
            FileError::VarError(ve) => SpeedrunSettingsFileError::VarError(ve),
            FileError::Other(msg) => SpeedrunSettingsFileError::Other(msg),
        }
    }
}

impl<'a> From<toml::de::Error> for ConfigurationFileError {
    fn from(e: toml::de::Error) -> Self {
        ConfigurationFileError::Deserialize(e)
    }
}

impl fmt::Display for SpeedrunSettingsFileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SpeedrunSettingsFileError::UserInput(msg) | SpeedrunSettingsFileError::Other(msg) => {
                writeln!(f, "{msg}")
            }
            SpeedrunSettingsFileError::UserCancel() => writeln!(
                f,
                "User cancelled action while editing speedrun settings file"
            ),
            SpeedrunSettingsFileError::OSStringConversion(msg) => writeln!(f, "{msg}"),
            SpeedrunSettingsFileError::VarError(ve) => fmt::Display::fmt(ve, f),
            SpeedrunSettingsFileError::Dialog(de) => fmt::Display::fmt(de, f),
            SpeedrunSettingsFileError::Serialize(se) => fmt::Display::fmt(se, f),
            SpeedrunSettingsFileError::Deserialize(de) => fmt::Display::fmt(de, f),
            SpeedrunSettingsFileError::IO(ioe) => fmt::Display::fmt(ioe, f),
            SpeedrunSettingsFileError::Missing(filename) => {
                writeln!(f, "Missing settings file with name: \"{filename}\"")
            }
        }
    }
}

impl<'a> fmt::Display for RunFileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RunFileError::UserInput(msg) | RunFileError::Other(msg) => writeln!(f, "{msg}"),
            // NOTE: should be unreachable
            RunFileError::UserCancel() => {
                writeln!(f, "User cancelled his action while editting run file")
            }
            RunFileError::Save(se) => fmt::Display::fmt(se, f),
            RunFileError::Parse(ce) => fmt::Display::fmt(ce, f),
            RunFileError::IO(msg) => fmt::Display::fmt(msg, f),
            RunFileError::VarError(ve) => fmt::Display::fmt(ve, f),
            RunFileError::Dialog(de) => fmt::Display::fmt(de, f),
        }
    }
}

impl<'a> fmt::Display for FileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FileError::UserInput(msg) | FileError::Other(msg) => writeln!(f, "{msg}"),
            FileError::UserCancel() => writeln!(f, "User cancelled his action"),
            FileError::VarError(ve) => fmt::Display::fmt(ve, f),
            FileError::Dialog(de) => fmt::Display::fmt(de, f),
            FileError::IO(ioe) => fmt::Display::fmt(ioe, f),
        }
    }
}

impl<'a> UserKeybinding<'_> {
    /// Represents keybindings provided by the user
    #[must_use]
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
/// Returns "$HOME/.config/.pmls" expanded
fn default_config_path() -> Result<String, ConfigurationFileError> {
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
    let binary_name = env!("CARGO_PKG_NAME");
    Ok(format!("{home}/.config/.{binary_name}"))
}

/// Returns "$HOME/.pmls" expanded
///
/// # Errors
/// Returns an error if the $HOME variable cannot be infered
pub fn default_data_folder() -> Result<String, FileError> {
    // Note: if executed with sudo, home will default to /root, which is usually not desired
    let home = std::env::var("HOME")?;
    let binary_name = env!("CARGO_PKG_NAME");
    Ok(format!("{home}/.{binary_name}"))
}

/// Returns "$HOME/.pmls/logs.txt"
///
/// # Errors
/// Returns an error if the $HOME variable cannot be infered
pub fn default_log_file_path() -> Result<String, FileError> {
    let home = std::env::var("HOME")?;
    let binary_name = env!("CARGO_PKG_NAME");
    Ok(format!("{home}/.{binary_name}/logs.txt"))
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
    #[must_use]
    pub fn get_game_name(&self) -> String {
        self.game_name.clone()
    }

    /// Return the name of the game category for this speedrun
    #[must_use]
    pub fn get_category_name(&self) -> String {
        self.category_name.clone()
    }

    /// Return names of splits for this speedrun
    #[must_use]
    pub fn get_split_names(&self) -> Vec<String> {
        self.split_names.clone()
    }

    /// Get split key from this speedrun settings
    #[must_use]
    pub fn get_split_key(&self) -> KeyCode {
        self.keybindings.split_key
    }

    /// Get reset key from this speedrun settings
    #[must_use]
    pub fn get_reset_key(&self) -> KeyCode {
        self.keybindings.reset_key
    }

    /// Get pause key from this speedrun settings
    #[must_use]
    pub fn get_pause_key(&self) -> KeyCode {
        self.keybindings.pause_key
    }

    /// Get unpause key from this speedrun settings
    #[must_use]
    pub fn get_unpause_key(&self) -> KeyCode {
        self.keybindings.unpause_key
    }

    /// Get comparison key from this speedrun settings
    #[must_use]
    pub fn get_comparison_key(&self) -> KeyCode {
        self.keybindings.comparison_key
    }
}

/// Parse configuration file at default path and ask user if not present
///
/// Accept and skip dialog if `accept_automatically_configuration_creation` is
/// `true`.
///
/// # Errors
/// Returns an error when user cancels filling settings or any issue occurs
/// with the dialog box
pub fn parse_configuration(
    accept_automatically_configuration_creation: bool,
) -> Result<PMLSConfiguration, ConfigurationFileError> {
    let default_config_path = default_config_path()?;
    let default_data_folder = default_data_folder()?;
    if !Path::new(default_config_path.as_str()).exists() {
        if accept_automatically_configuration_creation {
            let config = PMLSConfiguration::new()?;
            match save_config_to_file(&config) {
                Ok(()) => return Ok(config),
                Err(e) => return Err(e),
            };
        }

        let choice = dialog::Question::new(format!("No configuration file was found at \"{default_config_path}\". Create configuration file?").as_str())
    .title("Create configuration file?")
    .show()?;
        if choice == dialog::Choice::Yes {
            let config = PMLSConfiguration::new()?;
            match save_config_to_file(&config) {
                Ok(()) => return Ok(config),
                Err(e) => return Err(e),
            };
        }
        return Err(ConfigurationFileError::UserCancel());
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
    let config: PMLSConfiguration = toml::from_str(config.as_str())?;
    Ok(config)
}

/// Load speedrun from file and return true if newly created. If provided, returns filepath of
/// icons
///
/// Use user provided `game_name` and `category_name` if present. Otherwise,
/// fall back to default speedrun
///
/// # Errors
/// Returns an error when speedrun settings cannot be loaded, user provided
/// arguments are invalid or when user stops filling speedrun settings
pub fn load_speedrun_settings<'a>(
    configuration: &'a PMLSConfiguration,
    game_name: Option<&str>,
    category_name: Option<&str>,
    split_names: Option<&str>,
    user_keybinding: UserKeybinding,
    icons: Option<Values>,
    force_speedrun_settings_creation: bool,
) -> Result<(SpeedrunSettings, Option<Vec<String>>, bool), SpeedrunSettingsFileError> {
    if !force_speedrun_settings_creation {
        // look for speedrun file using `game_name` and `category_name`
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
                return Ok((
                    find_speedrun_by_name(settings.get_file_name(), configuration)?,
                    None,
                    false,
                ));
            }
        }
    }

    if !force_speedrun_settings_creation && configuration.use_default_speedrun {
        info!("Loading default speedrun");
        match configuration.default_speedrun_name.clone() {
            Some(n) => Ok((find_speedrun_by_name(n, configuration)?, None, false)),
            None => {
                warn!("No default speedrun name was set. Have you set a default_speedrun_name entry in your configuration file?");
                match ask_speedrun_settings_to_user(
                    game_name,
                    category_name,
                    split_names,
                    &user_keybinding,
                    icons,
                ) {
                    Ok((ss, names)) => Ok((ss, Some(names), true)),
                    Err(e) => Err(e),
                }
            }
        }
    } else {
        match ask_speedrun_settings_to_user(
            game_name,
            category_name,
            split_names,
            &user_keybinding,
            icons,
        ) {
            Ok((ss, names)) => Ok((ss, Some(names), true)),
            Err(e) => Err(e),
        }
    }
}

/// Search data folder from `configuration` for speedrun with provided `name`
fn find_speedrun_by_name(
    name: String,
    configuration: &PMLSConfiguration,
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
    Err(SpeedrunSettingsFileError::Missing(name))
}

/// Get user output. Exit program if user exits dialog
fn get_user_output(input: &mut Input) -> Result<String, FileError> {
    match input.show() {
        Ok(i) => match i {
            Some(i) => Ok(i),
            // NOTE: either cancel option was chosen or dialog was closed
            None => Err(FileError::UserCancel()),
        },
        Err(e) => Err(FileError::Dialog(e)),
    }
}

/// Ask user for speedrun settings
fn ask_speedrun_settings_to_user(
    game_name: Option<&str>,
    category_name: Option<&str>,
    split_names: Option<&str>,
    keybinding: &UserKeybinding,
    icons: Option<Values>,
) -> Result<(SpeedrunSettings, Vec<String>), SpeedrunSettingsFileError> {
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

    let mut split_names: Vec<String> = get_splits(split_names.unwrap_or_default());
    while split_names.is_empty() {
        let sn = get_user_output(dialog::Input::new(
            "Please provide at least one split, each separated with '|' (for instance 'split1|split 2|split 3'):",
        ).title("Enter split names"))?;
        split_names = get_splits(sn.as_str());
    }

    let mut icon_filepaths: Vec<String> = vec![];
    if let Some(icons) = icons {
        for icon in icons {
            icon_filepaths.push(icon.to_string());
        }
    }
    if icon_filepaths.is_empty() {
        let choice = dialog::Question::new("Do you want to select icon images for splits?")
            .title("Add split icon images?")
            .show()?;
        if choice == dialog::Choice::Yes {
            for name in split_names.clone() {
                let i = ask_for_icons(&name)?;
                icon_filepaths.push(i);
            }
        }
    }

    loop {
        // NOTE: KeyCode does not implement display but Debug matches serialized string
        let split_key = match keybinding.split_key {
            Some(k) => parse_key(k)?,
            None => ask_user_keybinding("start/split", format!("{Numpad1:?}"))?,
        };
        let reset_key = match keybinding.reset_key {
            Some(k) => parse_key(k)?,
            None => ask_user_keybinding("reset", format!("{Numpad3:?}"))?,
        };
        let pause_key = match keybinding.pause_key {
            Some(k) => parse_key(k)?,
            None => ask_user_keybinding("pause", format!("{Numpad5:?}"))?,
        };
        let unpause_key = match keybinding.unpause_key {
            Some(k) => parse_key(k)?,
            None => ask_user_keybinding("unpause", format!("{Numpad7:?}"))?,
        };
        let comparison_key = match keybinding.comparison_key {
            Some(k) => parse_key(k)?,
            None => ask_user_keybinding("comparison", format!("{Numpad9:?}"))?,
        };

        let keys = vec![split_key, reset_key, pause_key, unpause_key];
        if keys.iter().all_unique() {
            let keybinding =
                Keybinding::new(split_key, reset_key, pause_key, unpause_key, comparison_key);
            let ss = SpeedrunSettings::new(split_names, game_name, category_name, keybinding)?;
            return Ok((ss, icon_filepaths));
        }
        warn!("No two keybinds can be the same. Retrying...");
    }
}

/// Ask user keybinding for `key` while displaying `help`
fn ask_user_keybinding(key_name: &str, example_keybind: String) -> Result<KeyCode, FileError> {
    let description = format!("Please provide the {key_name} key (example: \"{example_keybind}\", all possible values https://github.com/obv-mikhail/InputBot/blob/develop/src/public.rs):");
    let title = format!("Provide {key_name} key");
    let k = get_user_output(
        Input::new(description)
            .title(title)
            .default(example_keybind),
    )?;
    let k = parse_key(k.as_str())?;
    Ok(k)
}

/// Get splits from `raw_splits`
fn get_splits(raw_splits: &str) -> Vec<String> {
    raw_splits
        .split('|')
        .map(std::string::ToString::to_string)
        .filter(|s| !s.is_empty())
        .collect()
}

/// Enabled default speedrun present in `settings` by updating `configuration`
///
/// # Errors
/// This functions returns an error if it cannot serialize configuration or
/// write to file
pub fn update_configuration_with_default_speedrun(
    configuration: PMLSConfiguration,
    settings: &SpeedrunSettings,
    auto_accept: bool,
) -> Result<(), ConfigurationFileError> {
    let game_name = settings.game_name.as_str();
    let category_name = settings.category_name.as_str();
    if auto_accept {
        let mut config = configuration;
        config.use_default_speedrun = true;
        config.default_speedrun_name = Some(settings.get_file_name());
        save_config_to_file(&config)
    } else {
        let choice =
            dialog::Question::new(format!("Make \"{game_name}: {category_name}\" default?"))
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
}

/// Save `configuration` to file
fn save_config_to_file(configuration: &PMLSConfiguration) -> Result<(), ConfigurationFileError> {
    let mut file = File::create(default_config_path()?.as_str())?;
    let config_content = toml::to_string(&configuration)?;
    file.write_all(config_content.as_bytes())?;
    info!("Configuration file created");
    Ok(())
}

/// Save speedrun `settings` to file
///
/// # Errors
/// This functions returns an error if it cannot serialize configuration or
/// write to file
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
///
/// # Errors
/// This functions returns an error if it cannot serialize configuration or
/// write to file
pub fn save_run_to_file(run: &Run, settings: &SpeedrunSettings) -> Result<(), RunFileError> {
    let default_data_folder = default_data_folder()?;
    let file_path = format!("{default_data_folder}/{}", settings.get_run_file_name());
    let file = File::create(file_path)?;
    let writer = BufWriter::new(file);
    livesplit::save_run(run, writer)?;
    Ok(())
}

/// Parse run from data folder present in `settings`
///
/// # Errors
/// Returns an error if run file could not be parsed
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

/// Ask user for icon
fn ask_for_icons(icon_name: &str) -> Result<String, SpeedrunSettingsFileError> {
    let img_filepath = dialog::FileSelection::new("")
        .title(format!("Select \"{icon_name}\" icon file"))
        .show()?;

    match img_filepath {
        Some(f) => Ok(f),
        None => Err(SpeedrunSettingsFileError::UserInput(
            "No filepath was provided".to_string(),
        )),
    }
}

/// Parse `key`
fn parse_key(key: &str) -> Result<KeyCode, FileError> {
    Ok(key.parse::<KeyCode>()?)
}

impl<'a> From<()> for FileError {
    fn from(_: ()) -> Self {
        FileError::Other("Could not convert key".to_string())
    }
}
