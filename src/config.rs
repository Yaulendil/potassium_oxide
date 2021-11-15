use dirs::config_dir;
use std::{fs::File, io::{Error as ErrorIo, Read, Write}, path::PathBuf};
use std::fmt::{Display, Formatter};
use toml::de::Error as ErrorToml;


macro_rules! filename {($name:expr) => {concat!($name, ".toml")}}


/// Contents of the default configuration file.
const CONFIG_DEFAULT: &str = include_str!(filename!("../cfg_default"));
const CONFIG_PATH: &str = filename!(env!("CARGO_PKG_NAME"));


/// Locate the Path of the Config File.
fn find_path() -> Option<PathBuf> {
    let mut path: Option<PathBuf> = config_dir();

    if let Some(dir) = &mut path {
        dir.push(CONFIG_PATH);
    }

    path
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
                write!(f, "Failed to parse Configuration file: {}", err)
            }
        }
    }
}


#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ConfigAuth {
    pub username: String,
    pub client_key: String,
    pub oauth: String,
}


#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ConfigBot {
    pub admins: Vec<String>,
    pub blacklist: Vec<String>,
    pub helmet: usize,
    pub prefix: String,
    pub raise_limit: usize,
}


#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Config {
    pub auth: ConfigAuth,
    pub bot: ConfigBot,
}


impl Config {
    pub fn from_path(path: impl Into<PathBuf>) -> Result<Self, ConfigError> {
        let path: PathBuf = path.into();

        if path.exists() {
            info!("Using Configuration file: {}", path.to_string_lossy());
        } else {
            let mut file = File::create(&path)?;
            info!("New Configuration file created: {}", path.to_string_lossy());

            file.write_all(CONFIG_DEFAULT.as_bytes())?;
            file.flush()?;
        }

        let mut data = String::new();
        let mut file = File::open(&path)?;
        file.read_to_string(&mut data)?;

        Ok(toml::from_str(&data)?)
    }

    pub fn setup() -> Result<Self, ConfigError> {
        match find_path() {
            Some(path) => Self::from_path(path),
            None => Err(ConfigError::NoPath),
        }
    }
}


// impl<P: Into<PathBuf>> TryFrom<P> for Config {
//     type Error = ConfigError;
//
//     fn try_from(path: P) -> Result<Self, Self::Error> {
//         Self::from_path(path)
//     }
// }


impl TryFrom<PathBuf> for Config {
    type Error = ConfigError;

    fn try_from(path: PathBuf) -> Result<Self, Self::Error> {
        Self::from_path(path)
    }
}
