use crate::parse_key;
use dialog::{DialogBox, Input};
use inputbot::KeybdKey;
use inputbot::KeybdKey::{Numpad1Key, Numpad3Key, Numpad7Key, Numpad9Key};
use itertools::Itertools;
use livesplit_core::run::parser::composite;
use livesplit_core::run::saver::livesplit;
use livesplit_core::Run;
use log::*;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::BufReader;
use std::io::BufWriter;
use std::io::{Read, Write};
use std::path::Path;
use std::path::PathBuf;
use std::{fmt, fs};
use walkdir::WalkDir;

// Note: assumes linux environment
// Livesplit has an official release for windows so not it's not worth
// supporting windows
// TODO support macos
//
// Note: can't use $HOME as is
/// Returns "$HOME/.config/.speedrun_splits" expanded
fn default_config_path() -> Result<String, Error> {
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
    let home = std::env::var("HOME").map_err(|e| Error::User(format!("{e}")))?;
    Ok(format!("{home}/.config/.speedrun_splits"))
}

/// Returns "$HOME/.speedrun_splits" expanded
pub fn default_data_folder() -> Result<String, Error> {
    // Note: if executed with sudo, home will default to /root, which is usually not desired
    let home = std::env::var("HOME").map_err(|e| Error::User(format!("{e}")))?;
    Ok(format!("{home}/.speedrun_splits"))
}

/// Returns "$HOME/.speedrun_splits/logs.txt"
pub fn default_log_file_path() -> Result<String, Error> {
    let home = std::env::var("HOME").map_err(|e| Error::User(format!("{e}")))?;
    Ok(format!("{home}/.speedrun_splits/logs.txt"))
}

#[derive(Serialize, Deserialize)]
pub struct Configuration {
    data_folder_path: String,
    // open default speedrun
    use_default_speedrun: bool,
    default_speedrun_name: Option<String>,
}

impl Configuration {
    fn new(data_folder_path: String) -> Result<Configuration, Error> {
        Ok(Configuration {
            data_folder_path,
            default_speedrun_name: None,
            use_default_speedrun: true,
        })
    }
}

#[derive(Serialize, Deserialize)]
pub struct SpeedrunSettings {
    pub split_names: Vec<String>,
    pub game_name: String,
    pub category_name: String,
    pub split_key: String,
    pub reset_key: String,
    pub pause_key: String,
    pub unpause_key: String,
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
    ) -> Result<SpeedrunSettings, Error> {
        let keys = vec![split_key, reset_key, pause_key, unpause_key];
        if !keys.iter().all_unique() {
            return Err(Error::SpeedrunSettings(
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

#[derive(Debug, Eq, PartialEq, Hash)]
pub enum Error {
    ConfigFileOpen(String),
    SpeedrunSettings(String),
    ConfigFileRead(String),
    ConfigCreate(String),
    DataFolder(String),
    Run(String),
    User(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let default_config_path = default_config_path().map_err(|e| {
            error!("{e}");
            fmt::Error
        })?;
        let default_data_folder = default_data_folder().map_err(|e| {
            error!("{e}"); // can't bubble up an error message, so error is logged
            fmt::Error
        })?;
        let msg: String = match self {
            Error::ConfigFileOpen(msg) => {
                format!("Could not open configuration file ({default_config_path}): {msg}")
            }
            Error::ConfigFileRead(msg) => {
                format!("Could not read configuration file {default_config_path}: {msg}")
            }
            Error::DataFolder(msg) => {
                format!("Error while using data folder {default_data_folder}: {msg}",)
            }
            Error::SpeedrunSettings(msg) => {
                format!("Error while using speedrun settings file in {default_data_folder}: {msg}")
            }
            Error::ConfigCreate(msg) => {
                format!("Error while creating configuration: {msg}")
            }
            Error::Run(msg) => {
                format!("Error happened while saving run: {msg}")
            }
            Error::User(msg) => {
                format!("Error while configuring user files: {msg}")
            }
        };
        write!(f, "{msg}")
    }
}

impl SpeedrunSettings {
    /// Get file name of this (`self`) settings
    fn get_file_name(&self) -> String {
        format!("{}_{}.txt", self.game_name, self.category_name)
    }

    /// Get file name of this (`self`) settings
    fn get_run_file_name(&self) -> String {
        format!("{}_{}.lss", self.game_name, self.category_name)
    }
}

/// Parse configuration file at default path and ask user if not present
pub fn parse_config() -> Result<Configuration, Error> {
    let default_config_path = default_config_path()?;
    let default_data_folder = default_data_folder()?;
    if !Path::new(default_config_path.as_str()).exists() {
        let choice = dialog::Question::new(format!("No configuration file was found at \"{default_config_path}\". Create configuration file?").as_str())
    .title("Create configuration file?")
    .show()
    .expect("Could not display dialog box");
        if choice == dialog::Choice::Yes {
            let config = Configuration::new(default_data_folder)?;
            match save_config_to_file(&config).map_err(|e| Error::SpeedrunSettings(format!("{e}")))
            {
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
        .show()
        .expect("Could not display dialog box");
        if choice == dialog::Choice::Yes {
            fs::create_dir(default_data_folder.as_str())
                .map_err(|e| Error::DataFolder(format!("{e}")))?;

            // look for existing speedrun
            trace!("Parsing default data folder...");
            for entry in WalkDir::new(default_data_folder) {
                let entry = entry.map_err(|e| Error::DataFolder(e.to_string()))?;

                debug!("{}", entry.path().display())
            }
        }
    }

    trace!("Parsing configuration file");
    let mut file = File::open(default_config_path.as_str())
        .map_err(|e| Error::ConfigFileOpen(e.to_string()))?;
    let mut config = String::new();
    file.read_to_string(&mut config)
        .map_err(|e| Error::ConfigFileRead(e.to_string()))?;
    let config: Configuration =
        toml::from_str(config.as_str()).map_err(|e| Error::ConfigFileRead(e.to_string()))?;
    Ok(config)
}

/// Load speedrun from file and return true if newly created
pub fn load_speedrun_settings(
    config: &Configuration,
    game_name: Option<&str>,
    category_name: Option<&str>,
    split_names: Option<&str>,
    user_split_key: Option<&str>,
    user_reset_key: Option<&str>,
    user_pause_key: Option<&str>,
    user_unpause_key: Option<&str>,
) -> (Result<SpeedrunSettings, Error>, bool) {
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
                find_speedrun_by_name(settings.get_file_name(), config),
                false,
            );
        }
    }

    if config.use_default_speedrun {
        info!("Loading default speedrun");
        match config.default_speedrun_name.clone() {
            Some(n) => (find_speedrun_by_name(n, config), false),
            None => {
                warn!("No default speedrun name was set. Have you set a default_speedrun_name entry in your configuration file?");
                (
                    ask_speedrun_settings_to_user(
                        game_name,
                        category_name,
                        split_names,
                        user_split_key,
                        user_reset_key,
                        user_pause_key,
                        user_unpause_key,
                    ),
                    true,
                )
            }
        }
    } else {
        (
            ask_speedrun_settings_to_user(
                game_name,
                category_name,
                split_names,
                user_split_key,
                user_reset_key,
                user_pause_key,
                user_unpause_key,
            ),
            true,
        )
    }
}

/// Search data folder for speedrun with provided `name`
fn find_speedrun_by_name(name: String, config: &Configuration) -> Result<SpeedrunSettings, Error> {
    let data_folder_path = config.data_folder_path.as_str();
    if name.is_empty() {
        return Err(Error::SpeedrunSettings(
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
        let file_name = e
            .path()
            .file_name()
            .ok_or("could not get file name")
            .map_err(|e| Error::SpeedrunSettings(e.to_string()))?;
        let ss_entry = file_name
            .to_str()
            .ok_or("Error converting file name")
            .map_err(|e| Error::SpeedrunSettings(e.to_string()))?;
        debug!("{ss_entry}");
        if ss_entry == name {
            info!("Found speedrun");
            trace!("Parsing speedrun settings file");
            let mut file = File::open(ss_file.as_str())
                .map_err(|e| Error::SpeedrunSettings(format!("{e}")))?;
            let mut ss_settings = String::new();
            file.read_to_string(&mut ss_settings)
                .map_err(|e| Error::SpeedrunSettings(format!("{e}")))?;
            let ss: SpeedrunSettings = toml::from_str(&ss_settings)
                .map_err(|e| Error::SpeedrunSettings(format!("{e}")))?;
            return Ok(ss);
        }
    }
    Err(Error::SpeedrunSettings(format!(
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
            return Err(Error::SpeedrunSettings(format!("{e}")));
        }
    }
}

/// Ask user for speedrun settings
fn ask_speedrun_settings_to_user(
    game_name: Option<&str>,
    category_name: Option<&str>,
    split_names: Option<&str>,
    user_split_key: Option<&str>,
    user_reset_key: Option<&str>,
    user_pause_key: Option<&str>,
    user_unpause_key: Option<&str>,
) -> Result<SpeedrunSettings, Error> {
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
        let split_key = match user_split_key {
            Some(k) => {
                parse_key(k.to_string()).map_err(|e| Error::SpeedrunSettings(format!("{e}")))?
            }
            None => ask_user_keybinding("start/split", format!("{:?}", Numpad1Key).as_str())?,
        };
        let reset_key = match user_reset_key {
            Some(k) => {
                parse_key(k.to_string()).map_err(|e| Error::SpeedrunSettings(format!("{e}")))?
            }
            None => ask_user_keybinding("reset", format!("{:?}", Numpad3Key).as_str())?,
        };
        let pause_key = match user_pause_key {
            Some(k) => {
                parse_key(k.to_string()).map_err(|e| Error::SpeedrunSettings(format!("{e}")))?
            }
            None => ask_user_keybinding("pause", format!("{:?}", Numpad7Key).as_str())?,
        };
        let unpause_key = match user_unpause_key {
            Some(k) => {
                parse_key(k.to_string()).map_err(|e| Error::SpeedrunSettings(format!("{e}")))?
            }
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
            warn!("Provided split and reset key cannot be the same. Retrying...")
        }
    }
}

/// Ask user keybinding for `key` while displaying `help`
fn ask_user_keybinding(key_name: &str, example_keybind: &str) -> Result<KeybdKey, Error> {
    let k = get_user_output(&mut Input::new(format!(
"Please provide the {key_name} key (example: \"{example_keybind}\", all possible values https://github.com/obv-mikhail/InputBot/blob/develop/src/public.rs):",
            )).title(format!("Provide {key_name} key")))?;

    let k = parse_key(k).map_err(|e| Error::User(e.to_string()))?;
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

/// Ask user if he wants this speedrun to be default. If yes, update configuration
pub fn update_config_with_default_speedrun(
    config: Configuration,
    settings: &SpeedrunSettings,
) -> Result<(), Error> {
    let game_name = settings.game_name.as_str();
    let category_name = settings.category_name.as_str();
    let choice = dialog::Question::new(format!("Make \"{game_name}: {category_name}\" default?"))
        .title("Make speedrun default?")
        .show()
        .expect("Could not display dialog box");
    if choice == dialog::Choice::Yes {
        let mut config = config;
        config.use_default_speedrun = true;
        config.default_speedrun_name = Some(settings.get_file_name());
        save_config_to_file(&config)
    } else {
        Ok(())
    }
}

/// Save `configuration` to file
fn save_config_to_file(configuration: &Configuration) -> Result<(), Error> {
    let mut file = File::create(default_config_path()?.as_str())
        .map_err(|e| Error::ConfigFileOpen(format!("{e}")))?;
    let config_content =
        toml::to_string(&configuration).map_err(|e| Error::ConfigCreate(format!("{e}")))?;
    file.write_all(config_content.as_bytes())
        .map_err(|e| Error::SpeedrunSettings(format!("{e}")))?;
    info!("Configuration file created");
    Ok(())
}

/// Save speedrun settings to file
pub fn save_speedrun_settings_to_file(settings: &SpeedrunSettings) -> Result<(), Error> {
    let default_data_folder = default_data_folder()?;
    let file_path = format!("{}/{}", default_data_folder, settings.get_file_name());
    let mut file = File::create(file_path).map_err(|e| Error::ConfigFileOpen(format!("{e}")))?;
    let settings_content =
        toml::to_string(&settings).map_err(|e| Error::SpeedrunSettings(e.to_string()))?;
    file.write_all(settings_content.as_bytes())
        .map_err(|e| Error::SpeedrunSettings(e.to_string()))?;

    info!("Speedrun settings file saved");
    Ok(())
}

/// Save `run` to file that corresponds to speedrun `settings`
pub fn save_run_to_file(run: &Run, settings: &SpeedrunSettings) -> Result<(), Error> {
    let default_data_folder = default_data_folder()?;
    let file_path = format!("{default_data_folder}/{}", settings.get_run_file_name());
    let file = File::create(file_path).map_err(|e| Error::Run(format!("{e}")))?;
    let writer = BufWriter::new(file);
    livesplit::save_run(run, writer).map_err(|e| Error::Run(format!("{e}")))
}

/// Parse run from data folder
pub fn parse_run_from_file(settings: &SpeedrunSettings) -> Result<Run, Error> {
    // Load the file.
    let default_data_folder = default_data_folder()?;
    let file_path = PathBuf::from(format!(
        "{default_data_folder}/{}",
        settings.get_run_file_name()
    ));
    let file = File::open(&file_path).map_err(|e| Error::Run(format!("{e}")))?;
    let file = BufReader::new(file);

    // We want to load additional files from the file system, like segment icons.
    let load_files = true;

    // Actually parse the file.
    let result = composite::parse(file, Some(file_path), load_files);
    let parsed = result.map_err(|e| Error::Run(format!("Not a valid splits file: {e}")))?;

    // Print out the detected file format.
    info!("Splits File Format: {}", parsed.kind);

    // Get out the Run object.
    let run = parsed.run;
    Ok(run)
}
