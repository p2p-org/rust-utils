#[cfg(feature = "db")]
pub mod db;
#[cfg(feature = "error")]
pub mod error;
#[cfg(feature = "logger")]
pub mod logger;
#[cfg(feature = "macros")]
pub mod macros;
#[cfg(feature = "settings")]
pub mod settings;
#[cfg(feature = "telemetry")]
pub mod telemetry;
#[cfg(feature = "tokens")]
pub mod tokens;

#[cfg(feature = "crypto")]
pub mod crypto;

#[cfg(feature = "settings")]
pub extern crate config;

#[cfg(feature = "settings")]
pub extern crate paste;
