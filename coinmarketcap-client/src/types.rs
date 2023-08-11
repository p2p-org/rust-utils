use std::collections::HashMap;

use anyhow::{anyhow, bail, Result};
use chrono::{DateTime, Utc};
use normdecimal::NormDecimal;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Quotes {
    #[serde(alias = "last_updated")]
    pub timestamp: DateTime<Utc>,
    #[serde(default)]
    pub quotes: Option<HashMap<String, Vec<Price>>>,
    #[serde(default)]
    pub quote: Option<HashMap<String, Price>>,
}

#[derive(Deserialize)]
pub struct Price {
    pub price: NormDecimal,
}

#[derive(Debug, Deserialize)]
pub struct Status {
    error_code: u32,
    error_message: String,
    // These fields can be used later
    //timestamp: DateTime<Utc>,
    //elapsed: u32,
    //credit_count: u32,
}

pub type CoinId = String;

#[derive(Deserialize)]
pub struct PricesResponse {
    #[serde(default)]
    data: Option<HashMap<CoinId, Vec<Quotes>>>,
    status: Status,
}

impl PricesResponse {
    pub fn is_error(&self) -> bool {
        self.status.error_code != 0
    }

    pub fn into_data(self) -> Result<HashMap<CoinId, Vec<Quotes>>> {
        self.error()?;
        self.data.ok_or_else(|| anyhow!("No data in response"))
    }

    fn error(&self) -> Result<()> {
        if self.is_error() {
            bail!(
                "Coinmarketcap error {}: {}",
                self.status.error_code,
                self.status.error_message
            );
        }
        Ok(())
    }
}
