use directories::ProjectDirs;
use std::{
    fmt::{Display, Formatter},
    fs::{create_dir, File},
    io::{Error as ErrorIo, Read, Write},
    path::{Path, PathBuf},
};
use toml::{de::Error as ErrorToml, map::Map, Value};


macro_rules! filename {($name:expr) => {concat!($name, ".toml")}}


/// Contents of the default configuration file.
const CONFIG_DEFAULT: &str = include_str!(filename!("cfg_default"));
const CONFIG_PATH: &str = filename!("cfg");
const CONFIG_SIZE: usize = 2048;


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
    pub username: String,
    // pub client_key: String,
    pub oauth: String,
}


#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ConfigBot {
    pub prefix: String,

    pub admins: Vec<String>,
    pub blacklist: Vec<String>,

    pub default_duration: u64,
    pub default_minimum: usize,

    pub helmet: u64,
    pub raise_limit: usize,
}


#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Config {
    pub auth: ConfigAuth,
    pub bot: ConfigBot,
    pub channel: Option<Map<String, Value>>,
}


macro_rules! impl_getter {
    ($($field:ident: $rtype:ty;)*) => {$(
        pub fn $field(&self, channel: &str) -> $rtype {
            match self.get(channel, stringify!($field)) {
                Some(value) => Value::try_into::<$rtype>(value.clone())
                    .unwrap_or(self.bot.$field),
                None => self.bot.$field,
            }
        }
    )*};
}


impl Config {
    fn get(&self, channel: &str, key: &str) -> Option<&Value> {
        self.channel.as_ref()?.get(channel)?.as_table()?.get(key)
    }

    impl_getter! {
        default_duration: u64;
        default_minimum: usize;
        helmet: u64;
        raise_limit: usize;
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
            file.flush()?;
        }

        let mut data = String::with_capacity(CONFIG_SIZE);
        { File::open(&path)?.read_to_string(&mut data)? };

        let mut new: Config = toml::from_str(&data)?;
        new.lower();

        // dbg!(&new.channel);

        Ok(new)
    }

    pub fn setup() -> Result<Self, ConfigError> {
        match find_path(true) {
            Some(path) => Self::from_path(path),
            None => Err(ConfigError::NoPath),
        }
    }
}


impl Config {
    pub fn lower(&mut self) {
        lower(&mut self.bot.admins);
        lower(&mut self.bot.blacklist);
    }
}
