use config::{Config, ConfigError};
use serde::de::DeserializeOwned;
use thiserror::Error;

pub static DEFAULT_SETTINGS_FILE: &str = "settings.toml";

/// Returns settings file name from first argument (args[1]) or a default file name "settings.toml"
/// #[deprecated(note = "use impl_settings")]
pub fn get_settings_file() -> String {
    std::env::args()
        .nth(1)
        .unwrap_or_else(|| DEFAULT_SETTINGS_FILE.to_owned())
}

/// #[deprecated(note = "use impl_settings")]
pub fn try_read_config<T, E>(env_prefix: &str) -> Result<T, E>
where
    T: DeserializeOwned,
    E: From<ConfigError>,
{
    let file = get_settings_file();
    try_read_file_config(&file, env_prefix)
}

/// #[deprecated(note = "use impl_settings")]
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

/// #[deprecated(note = "use impl_settings")]
pub fn read_config_or_default<T>(env_prefix: &str) -> T
where
    T: DeserializeOwned + Default,
{
    let file = get_settings_file();
    read_file_config_or_default(&file, env_prefix)
}

/// #[deprecated(note = "use impl_settings")]
pub fn read_config_or_fail<T>(env_prefix: &str) -> T
where
    T: DeserializeOwned,
{
    let file = get_settings_file();
    read_file_config_or_fail(&file, env_prefix)
}

/// #[deprecated(note = "use impl_settings")]
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

/// #[deprecated(note = "use impl_settings")]
pub fn read_file_config_or_fail<T>(file: &str, env_prefix: &str) -> T
where
    T: DeserializeOwned,
{
    try_read_file_config::<T, ConfigError>(file, env_prefix).expect("unable to read config")
}

#[derive(Debug, Error)]
pub enum SettingsError {
    #[error("Config error: {0}")]
    Config(#[from] config::ConfigError),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("bad JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("bad application secret")]
    BadSecret,
}

/// Macro for simple initialization of Settings structures.
/// The struct inside macro define as a common way, but with little improvement. You should to type
/// default value after the type with separator `=>` for example `pub field_name: TypeName => <default_value>`
///
/// # Example:
/// ```ignore
/// impl_settings! {
///     #[derive(Debug, Deserialize, PartialEq, Eq)]
///     pub struct ExampleSettings {
///         #[serde(default = "ExampleSettings::default_field_1")]
///         pub some_u8_field: u8 => 1,
///
///         #[serde(default = "ExampleSettings::default_field_2")]
///         pub some_string_field: String => "hello I'm example settings".into(),
///
///         #[serde(default = "ExampleSettings::default_logger")]
///         pub logger: LoggerSettings => LoggerSettings::default()
///     }
/// }
///
/// fn main() {
///     let example_settings = ExampleSettings::new();
///
///     assert_eq!(example_settings.some_u8_field, 1);
///     assert_eq!(&example_settings.some_string_field, "hello I'm example settings");
///     assert_eq!(example_settings.logger, LoggerSettings::default());
/// }
/// ```
#[macro_export]
macro_rules! impl_settings {
    {$(
        $( #[ $attr:meta ] )*
        $vis:vis struct $name:ident { $(
            $( #[ $childm:meta ] )*
            $vis_f:vis $field:ident: $type:ident => $def:expr
        ),* $(,)?}
    )*} => {$(
        #[allow(unused_qualifications)]
        #[serde_with::serde_as]
        $(#[$attr])*
        $vis struct $name { $(
            $( #[$childm] )*
            $vis_f $field: $type,
        )+}

        $crate::paste::paste! {
            impl Default for $name {
                fn default() -> Self {
                    Self {
                        $(
                            $field: Self::[<default_ $field>](),
                        )+
                    }
                }
            }
        }

        $crate::paste::paste! {
            impl $name {
                $(
                    fn [<default_ $field>]() -> $type {
                        $def
                    }
                )+
            }
        }

        impl $name {
            fn default_settings_file() -> String {
                "settings.toml".into()
            }

            /// Returns settings file name from first argument (args[1]) or a default file name "settings.toml"
            pub fn get_settings_file() -> String {
                std::env::args()
                    .nth(1)
                    .unwrap_or_else(|| Self::default_settings_file())
            }


            pub fn try_read_config<E>(env_prefix: &str) -> Result<Self, E>
            where
                E: From<$crate::config::ConfigError>,
            {
                let file = Self::get_settings_file();
                Self::try_read_file_config(&file, env_prefix)
            }


            pub fn try_read_file_config<E>(file: &str, env_prefix: &str) -> Result<Self, E>
            where
                E: From<$crate::config::ConfigError>,
            {
                $crate::config::Config::builder()
                    .add_source($crate::config::File::with_name(file).required(false))
                    .add_source($crate::config::Environment::with_prefix(env_prefix)
                    .separator("__"))
                    .build()
                    .and_then($crate::config::Config::try_deserialize)
                    .map_err(Into::into)
            }

            #[allow(dead_code)]
            pub fn try_new() -> Result<Self, $crate::settings::SettingsError> {
                Self::try_read_config(APP_ENV_PREFIX)
            }

            #[cfg(not(crate_name = "rust-utils"))]
            pub fn new() -> Self {
                Self::try_read_config::<$crate::settings::SettingsError>(APP_ENV_PREFIX).unwrap_or_default()
            }
        }
    )*};
}

/// Macro for simple initialization of DbSettings structures by DB url.
///
/// # Example:
/// ```ignore
/// impl_db_settings! { "https://example.url" }
/// ```
#[macro_export]
macro_rules! impl_db_settings {
    { $default_url:expr } => {
    #[serde_with::serde_as]
    #[derive(Debug, Deserialize, PartialEq, Eq)]
    pub struct DbSettings {
        #[serde(default = "DbSettings::default_url")]
        pub url: String,
        #[serde(default = "DbSettings::default_pool_size")]
        pub pool_size: u32,
        #[serde(rename = "connect_timeout_ms", default = "DbSettings::default_connect_timeout")]
        #[serde_as(as = "serde_with::DurationMilliSeconds")]
        pub connect_timeout: std::time::Duration,
    }

    impl Default for DbSettings {
        fn default() -> Self {
            Self {
                url: Self::default_url(),
                pool_size: Self::default_pool_size(),
                connect_timeout: Self::default_connect_timeout(),
            }
        }
    }

    impl DbSettings {
        fn default_url() -> String {
            String::from($default_url)
        }

        fn default_pool_size() -> u32 {
            10
        }

        fn default_connect_timeout() -> std::time::Duration {
            std::time::Duration::from_secs(60)
        }
    }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use crate::logger::LoggerSettings;
    use lazy_static::lazy_static;
    use serde::Deserialize;

    lazy_static! {
        pub static ref NO_PARALLEL_TEST: Mutex<()> = Mutex::new(());
    }

    pub static APP_ENV_PREFIX: &str = "TESTS";

    fn default_field_1() -> u8 {
        1
    }

    fn default_field_2() -> String {
        "Hello world".into()
    }

    static DB_URL: &str = "https://test_url.com";
    impl_db_settings! { DB_URL }

    impl_settings! {
        #[derive(Debug, Deserialize, PartialEq, Eq)]
        pub struct TestSettings {
            #[serde(default = "TestSettings::default_field_1")]
            pub field_1: u8 => default_field_1(),

            #[serde(default = "TestSettings::default_field_2")]
            pub field_2: String => default_field_2(),

            #[serde(default = "TestSettings::default_logger")]
            pub logger: LoggerSettings => LoggerSettings::default(),

            #[serde(default = "TestSettings::default_db_settings")]
            pub db_settings: DbSettings => DbSettings::default()
        }
    }

    #[test]
    fn check_default() {
        let _locker = NO_PARALLEL_TEST.lock();
        let default_settings = TestSettings::default();

        let expected_settings = TestSettings {
            field_1: default_field_1(),
            field_2: default_field_2(),
            logger: LoggerSettings::default(),
            db_settings: DbSettings::default(),
        };

        assert_eq!(expected_settings, default_settings);
        assert_eq!(&expected_settings.db_settings.url, DB_URL);
    }

    #[test]
    fn check_from_env() {
        let _locker = NO_PARALLEL_TEST.lock();
        std::env::set_var("TESTS__field_1", "2");
        std::env::set_var("TESTS__field_2", "Hello from environment");

        let settings = TestSettings::new();

        std::env::remove_var("TESTS__field_1");
        std::env::remove_var("TESTS__field_2");

        let expected_settings = {
            TestSettings {
                field_1: 2,
                field_2: "Hello from environment".into(),
                logger: LoggerSettings::default(),
                db_settings: DbSettings::default(),
            }
        };

        assert_eq!(expected_settings, settings);
    }

    #[test]
    fn check_from_env_only_one_field() {
        let _locker = NO_PARALLEL_TEST.lock();
        std::env::set_var("TESTS__field_1", "2");

        let settings = TestSettings::new();

        std::env::remove_var("TESTS__field_1");

        let expected_settings = {
            TestSettings {
                field_1: 2,
                field_2: default_field_2(),
                logger: LoggerSettings::default(),
                db_settings: DbSettings::default(),
            }
        };

        assert_eq!(expected_settings, settings);
    }
}
