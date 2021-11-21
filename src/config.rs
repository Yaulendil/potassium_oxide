use directories::ProjectDirs;
use std::{
    fs::{create_dir, File, rename},
    io::{Error as ErrorIo, Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};
use toml::{de::Error as ErrorToml, map::Map, Value};
use twitchchat::twitch::{UserConfig, UserConfigError};


macro_rules! filename {($name:expr) => {concat!($name, ".toml")}}


/// Contents of the default configuration file.
const CONFIG_DEFAULT: &str = include_str!(filename!("cfg_default"));
const CONFIG_PATH: &str = filename!("cfg");
const CONFIG_SIZE: usize = 2048;


fn contains<I, T, U>(sequence: I, want: U) -> bool where
    I: IntoIterator<Item=T>,
    T: PartialEq<U>,
{
    sequence.into_iter().any(|item: T| item == want)
}


/// Locate the Path of the Config File.
fn find_path(create_missing: bool) -> Option<PathBuf> {
    let dirs: ProjectDirs = ProjectDirs::from("", "", env!("CARGO_PKG_NAME"))?;
    let mut path: PathBuf = dirs.config_dir().to_owned();

    if create_missing && !path.exists() {
        create_dir(&path).ok()?;
    }

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
    FileInaccessible(ErrorIo),
    FileInvalid(ErrorToml),
    FileValid(Config),
}


#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ConfigAuth {
    username: String,
    oauth: String,
}


#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ConfigAdmin {
    admins: Vec<String>,
    blacklist: Vec<String>,

    prefix: String,
    reconnect: u64,
}


#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ConfigAuction {
    duration: u64,
    helmet: u64,

    max_raise: usize,
    min_bid: usize,
}


#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Config {
    auth: ConfigAuth,
    admin: ConfigAdmin,
    auction: ConfigAuction,

    channel: Option<Map<String, Value>>,
}


impl Config {
    pub fn create(path: &Path, create_parent: bool) -> Result<(), ErrorIo> {
        if let Some(parent) = path.parent() {
            if create_parent && !parent.exists() {
                create_dir(parent)?;
            }
        }

        if path.exists() {
            if let Some(backup) = get_backup(path) {
                rename(path, backup).ok();
            }
        }

        File::create(path)?.write_all(CONFIG_DEFAULT.as_bytes())
    }

    pub fn find(path_opt: Option<PathBuf>) -> ConfigFind {
        use ConfigFind::*;

        match path_opt.or_else(|| find_path(false)) {
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

        let mut data = String::with_capacity(CONFIG_SIZE);
        if let Err(e) = File::open(&path).and_then(|mut file: File| {
            file.read_to_string(&mut data)
        }) {
            return FileInaccessible(e);
        }

        match toml::from_str::<Config>(&data) {
            Err(e) => FileInvalid(e),
            Ok(mut config) => {
                config.lower();
                FileValid(config)
            }
        }
    }
}


/// Methods for retrieving configuration data without exposing the Struct.
impl Config {
    fn get(&self, channel: &str, key: &str) -> Option<&Value> {
        self.channel.as_ref()?.get(channel)?.as_table()?.get(key)
    }

    pub fn get_auth(&self) -> Result<UserConfig, UserConfigError> {
        UserConfig::builder()
            .name(&self.auth.username)
            .token(&self.auth.oauth)
            .enable_all_capabilities()
            .build()
    }

    pub fn get_duration(&self, channel: &str) -> Duration {
        Duration::from_secs(match self.get(channel, "duration").cloned() {
            Some(value) => value.try_into().unwrap_or(self.auction.duration),
            None => self.auction.duration,
        })
    }

    pub fn get_helmet(&self, channel: &str) -> Duration {
        Duration::from_secs(match self.get(channel, "helmet").cloned() {
            Some(value) => value.try_into().unwrap_or(self.auction.helmet),
            None => self.auction.helmet,
        })
    }

    pub fn get_max_raise(&self, channel: &str) -> usize {
        match self.get(channel, "max_raise").cloned() {
            Some(value) => value.try_into().unwrap_or(self.auction.max_raise),
            None => self.auction.max_raise,
        }
    }

    pub fn get_min_bid(&self, channel: &str) -> usize {
        match self.get(channel, "min_bid").cloned() {
            Some(value) => value.try_into().unwrap_or(self.auction.min_bid),
            None => self.auction.min_bid,
        }
    }

    pub fn get_prefix(&self) -> &str {
        &self.admin.prefix
    }

    pub const fn get_reconnect(&self) -> Duration {
        Duration::from_secs(self.admin.reconnect)
    }

    pub fn is_admin(&self, name: &str) -> bool {
        contains(&self.admin.admins, name)
    }

    pub fn is_blacklisted(&self, name: &str) -> bool {
        contains(&self.admin.blacklist, name)
    }
}


impl Config {
    pub fn lower(&mut self) {
        lower(&mut self.admin.admins);
        lower(&mut self.admin.blacklist);
    }
}
