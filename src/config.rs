use directories::ProjectDirs;
use std::{
    fmt::{Display, Formatter},
    fs::{create_dir, File},
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


fn lower(vec: &mut Vec<String>) {
    for name in vec.iter_mut() {
        name.make_ascii_lowercase();
    }
}


#[derive(Debug)]
pub enum ConfigError {
    Io(ErrorIo),
    NoPath,
    ParseError(ErrorToml),
}

impl ConfigError {
    pub fn status(&self) -> i32 {
        match self {
            ConfigError::Io(err) => err.raw_os_error().unwrap_or(1),
            _ => 1,
        }
    }
}


impl From<ErrorIo> for ConfigError {
    fn from(e: ErrorIo) -> Self { Self::Io(e) }
}


impl From<ErrorToml> for ConfigError {
    fn from(e: ErrorToml) -> Self { Self::ParseError(e) }
}


impl Display for ConfigError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::Io(err) => {
                write!(f, "Failed to access Configuration file: {}", err)
            }
            ConfigError::NoPath => {
                write!(f, "Failed to find Configuration filepath.")
            }
            ConfigError::ParseError(err) => {
                write!(f, "Failed to parse Configuration: {}", err)
            }
        }
    }
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


impl Config {
    pub fn ensure(path_opt: Option<PathBuf>, force: bool) -> i32 {
        match path_opt.or_else(|| find_path(true)) {
            Some(path) if force || !path.exists() => {
                println!("Creating new Config file: {}", path.display());

                match File::create(&path) {
                    Ok(mut f) => match f.write_all(CONFIG_DEFAULT.as_bytes()) {
                        Ok(..) => {
                            println!("Default Config written successfully.");
                            0
                        }
                        Err(e) => {
                            println!("Failed to write default Config: {}", e);
                            1
                        }
                    }
                    Err(e) => {
                        println!("Failed to create file: {}", e);
                        1
                    }
                }
            }
            Some(path) => {
                println!("Found existing Config file: {}", path.display());
                0
            }
            None => {
                println!("Failed to find a path for the Config file.");
                1
            }
        }
    }

    pub fn find(path_opt: Option<PathBuf>) -> i32 {
        let opt: Option<PathBuf> = path_opt.or_else(|| find_path(false));

        match &opt {
            Some(path) if path.exists() => println!(
                "Found existing Config file: {}",
                path.display(),
            ),
            Some(path) => println!(
                "Config file does not exist: {}",
                path.display(),
            ),
            None => println!("Failed to find a path for the Config file."),
        }

        opt.is_none() as i32
    }

    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let path = path.as_ref();

        if path.exists() {
            info!("Using existing Config file: {}", path.display());
        } else {
            let mut file = File::create(&path)?;
            info!("New Config file created: {}", path.display());

            file.write_all(CONFIG_DEFAULT.as_bytes())?;
        }

        let mut data = String::with_capacity(CONFIG_SIZE);
        { File::open(&path)?.read_to_string(&mut data)? };

        let mut new: Config = toml::from_str(&data)?;
        new.lower();

        Ok(new)
    }

    pub fn setup() -> Result<Self, ConfigError> {
        match find_path(true) {
            Some(path) => Self::from_path(path),
            None => Err(ConfigError::NoPath),
        }
    }
}
