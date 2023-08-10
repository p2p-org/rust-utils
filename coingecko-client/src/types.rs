use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, FromRow, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
pub struct CoingeckoInfo {
    pub coin_id: String,
    pub name: String,
    pub symbol: String,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct CoingeckoInfoWithAddress {
    pub metadata: CoingeckoInfo,
    pub addresses: HashMap<String, String>, // Platform, address
}
