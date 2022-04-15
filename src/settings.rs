use config::{Config, ConfigError};
use serde::de::DeserializeOwned;

pub static DEFAULT_SETTINGS_FILE: &str = "settings.toml";

/// Returns settings file name from first argument (args[1]) or a default file name "settings.toml"
pub fn get_settings_file() -> String {
    std::env::args()
        .nth(1)
        .unwrap_or_else(|| DEFAULT_SETTINGS_FILE.to_owned())
}

pub fn try_read_config<T, E>(env_prefix: &str) -> Result<T, E>
where
    T: DeserializeOwned,
    E: From<ConfigError>,
{
    let file = get_settings_file();
    try_read_file_config(&file, env_prefix)
}

pub fn try_read_file_config<T, E>(file: &str, env_prefix: &str) -> Result<T, E>
where
    T: DeserializeOwned,
    E: From<ConfigError>,
{
    Config::builder()
        .add_source(config::File::with_name(file).required(false))
        .add_source(config::Environment::with_prefix(env_prefix).separator("__"))
        .build()
        .and_then(Config::try_deserialize)
        .map_err(Into::into)
}

pub fn read_config_or_default<T>(env_prefix: &str) -> T
where
    T: DeserializeOwned + Default,
{
    let file = get_settings_file();
    read_file_config_or_default(&file, env_prefix)
}

pub fn read_config_or_fail<T>(env_prefix: &str) -> T
where
    T: DeserializeOwned,
{
    let file = get_settings_file();
    read_file_config_or_fail(&file, env_prefix)
}

pub fn read_file_config_or_default<T>(file: &str, env_prefix: &str) -> T
where
    T: DeserializeOwned + Default,
{
    try_read_file_config::<T, ConfigError>(file, env_prefix)
        .map_err(|error| {
            log::warn!("config error: {error}, going on with default config...");
        })
        .unwrap_or_default()
}

pub fn read_file_config_or_fail<T>(file: &str, env_prefix: &str) -> T
where
    T: DeserializeOwned,
{
    try_read_file_config::<T, ConfigError>(file, env_prefix).expect("unable to read config")
}
