use std::{
    collections::HashMap,
    fs::{create_dir, File, rename},
    io::{Read, Seek, SeekFrom, Write},
    ops::{Deref, DerefMut},
    path::{Path, PathBuf},
    time::Duration,
};
use directories::ProjectDirs;
use twitchchat::twitch::{UserConfig, UserConfigError};


macro_rules! filename {($name:expr) => {concat!($name, ".toml")}}


/// Contents of the default configuration file.
const CONFIG_DEFAULT: &str = include_str!(filename!("cfg_default"));
const CONFIG_PATH: &str = filename!("cfg");


fn contains<I, T, U>(sequence: I, want: U) -> bool where
    I: IntoIterator<Item=T>,
    T: PartialEq<U>,
{
    sequence.into_iter().any(|item: T| item == want)
}


/// Locate the Path of the Config File.
fn find_path() -> Option<PathBuf> {
    let dirs: ProjectDirs = ProjectDirs::from("", "", env!("CARGO_PKG_NAME"))?;
    let mut path: PathBuf = dirs.config_dir().to_owned();

    path.push(CONFIG_PATH);
    Some(path)
}


fn get_backup(path: &Path) -> Option<PathBuf> {
    Some(path.with_file_name(
        format!(".bkp.{}", path.file_name()?.to_string_lossy())
    ))
}


fn lower(vec: &mut Vec<String>) {
    for name in vec.iter_mut() {
        name.make_ascii_lowercase();
    }
}


pub enum ConfigFind {
    DoesNotExist(PathBuf),
    Exists(PathBuf, ConfigOpen),
    NoPath,
}

impl ConfigFind {
    pub fn path(&self) -> Option<&PathBuf> {
        match self {
            Self::NoPath => None,
            Self::Exists(path, _)
            | Self::DoesNotExist(path) => Some(path),
        }
    }
}


pub enum ConfigOpen {
    FileInaccessible(std::io::Error),
    FileInvalid(toml::de::Error),
    FileValid(Config),
}


#[derive(Clone, Deserialize, Serialize)]
pub struct ConfigAuth {
    username: String,
    oauth: String,
}


#[derive(Clone, Deserialize, Serialize)]
pub struct ConfigAuction {
    duration: u64,
    helmet: u64,

    max_raise: usize,
    min_bid: usize,

    summary: bool,
    verb: String,
}


#[derive(Clone, Deserialize, Serialize)]
pub struct ConfigBot {
    #[serde(default)]
    admins: Vec<String>,
    #[serde(default, alias = "blacklist")]
    ignore: Vec<String>,

    parse_commands: bool,
    prefix: String,
    reconnect: u64,

    #[cfg(feature = "csv")]
    file_csv: Option<PathBuf>,
}


#[derive(Clone, Deserialize, Serialize)]
pub struct ConfigChannel {
    admins: Option<Vec<String>>,
    #[serde(alias = "blacklist")]
    ignore: Option<Vec<String>>,

    duration: Option<u64>,
    helmet: Option<u64>,

    max_raise: Option<usize>,
    min_bid: Option<usize>,

    summary: Option<bool>,
    verb: Option<String>,
}


#[derive(Clone, Deserialize, Serialize)]
pub struct Config {
    auth: ConfigAuth,
    auction: ConfigAuction,

    #[serde(alias = "admin")]
    bot: ConfigBot,

    #[serde(rename = "channel")]
    channels: Option<HashMap<String, ConfigChannel>>,
}


impl Config {
    pub fn create(path: &Path, create_parent: bool) -> Result<(), std::io::Error> {
        if path.exists() {
            if let Some(backup) = get_backup(path) {
                rename(path, backup).ok();
            }
        } else if create_parent {
            if let Some(parent) = path.parent() {
                if !parent.exists() {
                    create_dir(parent)?;
                }
            }
        }

        File::create(path)?.write_all(CONFIG_DEFAULT.as_bytes())
    }

    pub fn find(path_opt: Option<PathBuf>) -> ConfigFind {
        use ConfigFind::*;

        match path_opt.or_else(find_path) {
            None => NoPath,
            Some(path) if !path.exists() => DoesNotExist(path),
            Some(path) => {
                let open = Self::open(&path);
                Exists(path, open)
            }
        }
    }

    pub fn open(path: &Path) -> ConfigOpen {
        use ConfigOpen::*;

        let data = match File::open(&path) {
            Ok(mut file) => match file.seek(SeekFrom::End(0)) {
                Ok(len) => match file.rewind() {
                    Ok(..) => {
                        let mut buf = String::with_capacity(1 + len as usize);

                        match file.read_to_string(&mut buf) {
                            Ok(..) => buf,
                            Err(e) => { return FileInaccessible(e); }
                        }
                    }
                    Err(e) => { return FileInaccessible(e); }
                }
                Err(e) => { return FileInaccessible(e); }
            }
            Err(e) => { return FileInaccessible(e); }
        };

        match toml::from_str::<Config>(&data) {
            Err(e) => FileInvalid(e),
            Ok(mut config) => {
                config.lower();
                FileValid(config)
            }
        }
    }

    pub const fn with_path(self, path: PathBuf) -> ConfigFile {
        ConfigFile { data: self, path }
    }
}


/// Methods for retrieving configuration data without exposing the Struct.
impl Config {
    fn config_channel(&self, channel: &str) -> Option<&ConfigChannel> {
        self.channels.as_ref()?.get(channel)
    }

    pub fn auth(&self) -> Result<UserConfig, UserConfigError> {
        UserConfig::builder()
            .name(&self.auth.username)
            .token(&self.auth.oauth)
            .enable_all_capabilities()
            .build()
    }

    pub fn duration(&self, channel: &str) -> Duration {
        Duration::from_secs(match self.config_channel(channel) {
            Some(ConfigChannel { duration: Some(value), .. }) => *value,
            _ => self.auction.duration,
        })
    }

    #[cfg(feature = "csv")]
    pub const fn file_csv(&self) -> Option<&PathBuf> {
        self.bot.file_csv.as_ref()
    }

    pub fn helmet(&self, channel: &str) -> Duration {
        Duration::from_secs(match self.config_channel(channel) {
            Some(ConfigChannel { helmet: Some(value), .. }) => *value,
            _ => self.auction.helmet,
        })
    }

    pub fn max_raise(&self, channel: &str) -> usize {
        match self.config_channel(channel) {
            Some(ConfigChannel { max_raise: Some(value), .. }) => *value,
            _ => self.auction.max_raise,
        }
    }

    pub fn min_bid(&self, channel: &str) -> usize {
        match self.config_channel(channel) {
            Some(ConfigChannel { min_bid: Some(value), .. }) => *value,
            _ => self.auction.min_bid,
        }
    }

    pub const fn parse_commands(&self) -> bool {
        self.bot.parse_commands
    }

    pub const fn prefix(&self) -> &String {
        &self.bot.prefix
    }

    pub const fn reconnect(&self) -> Duration {
        Duration::from_secs(self.bot.reconnect)
    }

    pub fn summary(&self, channel: &str) -> bool {
        match self.config_channel(channel) {
            Some(ConfigChannel { summary: Some(value), .. }) => *value,
            _ => self.auction.summary,
        }
    }

    pub fn verb(&self, channel: &str) -> &str {
        match self.config_channel(channel) {
            Some(ConfigChannel { verb: Some(value), .. }) => value,
            _ => &self.auction.verb,
        }
    }
}


/// Methods for testing specific configured conditions.
impl Config {
    pub fn is_admin(&self, name: &str, channel: &str) -> bool {
        if self.is_globally_admin(name) {
            true
        } else {
            match self.config_channel(channel) {
                Some(ConfigChannel { admins: Some(list), .. })
                => contains(list, name),
                _ => false,
            }
        }
    }

    pub fn is_blacklisted(&self, name: &str, channel: &str) -> bool {
        if self.is_globally_blacklisted(name) {
            true
        } else {
            match self.config_channel(channel) {
                Some(ConfigChannel { ignore: Some(list), .. })
                => contains(list, name),
                _ => false,
            }
        }
    }

    pub fn is_globally_admin(&self, name: &str) -> bool {
        contains(&self.bot.admins, name)
    }

    pub fn is_globally_blacklisted(&self, name: &str) -> bool {
        contains(&self.bot.ignore, name)
    }
}


impl Config {
    pub fn lower(&mut self) {
        lower(&mut self.bot.admins);
        lower(&mut self.bot.ignore);
    }
}


#[derive(Clone)]
pub struct ConfigFile {
    data: Config,
    path: PathBuf,
}


impl ConfigFile {
    pub fn reload(&mut self) -> Result<(), ConfigOpen> {
        match Config::open(&self.path) {
            ConfigOpen::FileValid(new) => {
                self.data = new;
                Ok(())
            }
            err => Err(err),
        }
    }
}


impl Deref for ConfigFile {
    type Target = Config;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}


impl DerefMut for ConfigFile {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}
