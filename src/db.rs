use async_trait::async_trait;
use std::{
    ops::{Deref, DerefMut},
    time::Duration,
};

use serde::Deserialize;
use serde_with::{serde_as, DurationMilliSeconds};
use sqlx::{postgres::PgPoolOptions, Error, PgPool};

#[serde_as]
#[derive(Debug, Deserialize, Clone)]
pub struct DbSettings {
    pub url: String,
    #[serde(default = "DbSettings::default_pool_size")]
    pub pool_size: u32,
    #[serde(rename = "connect_timeout_ms", default = "DbSettings::default_connect_timeout")]
    #[serde_as(as = "DurationMilliSeconds")]
    pub connect_timeout: Duration,
}

impl DbSettings {
    fn default_pool_size() -> u32 {
        10
    }

    fn default_connect_timeout() -> Duration {
        Duration::from_secs(60)
    }
}

#[async_trait]
pub trait Repo {
    type Access: Access;
    async fn access(&self) -> Result<Self::Access, Error>;
}

#[async_trait]
pub trait Access {
    async fn done(self) -> Result<(), Error>;
}

pub struct DbRepo {
    pool: PgPool,
}

impl DbRepo {
    pub async fn connect(settings: &DbSettings) -> Result<Self, Error> {
        PgPoolOptions::new()
            .max_connections(settings.pool_size)
            .acquire_timeout(settings.connect_timeout)
            .connect(&settings.url)
            .await
            .map(Self::from)
    }
}

impl From<PgPool> for DbRepo {
    fn from(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl Repo for DbRepo {
    type Access = DbAccess;

    async fn access(&self) -> Result<Self::Access, sqlx::Error> {
        self.pool.begin().await.map(DbAccess)
    }
}

pub struct DbAccess(sqlx::Transaction<'static, sqlx::Postgres>);

#[async_trait]
impl Access for DbAccess {
    async fn done(self) -> Result<(), sqlx::Error> {
        self.0.commit().await
    }
}

impl Deref for DbAccess {
    type Target = sqlx::Transaction<'static, sqlx::Postgres>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for DbAccess {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
