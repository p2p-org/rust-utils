use std::{ops::Deref, sync::Arc, time::Duration};

use diesel::{
    mysql::MysqlConnection,
    r2d2::{ConnectionManager, Pool, PoolError, PooledConnection},
};
use scheduled_thread_pool::ScheduledThreadPool;
use serde::Deserialize;

pub type DbConnectionPool = Pool<ConnectionManager<MysqlConnection>>;

#[derive(Debug, Deserialize, Clone)]
pub struct DatabaseSettings {
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub connection_pool_size: u32,
    #[serde(default)]
    pub connection_ttl_secs: u64,
    #[serde(default)]
    pub connection_timeout_millis: u64,
    #[serde(default)]
    pub thread_pool_size: usize,
}

pub enum DbConnection {
    Simple(MysqlConnection),
    Pooled(DbConnectionPool),
}

impl DbConnection {
    pub fn get(&self) -> Result<DbConnectionRef, PoolError> {
        Ok(match self {
            DbConnection::Simple(conn) => DbConnectionRef::Simple(conn),
            DbConnection::Pooled(pool) => DbConnectionRef::Pooled(pool.get()?),
        })
    }
}

pub enum DbConnectionRef<'a> {
    Simple(&'a MysqlConnection),
    Pooled(PooledConnection<ConnectionManager<MysqlConnection>>),
}

impl Deref for DbConnectionRef<'_> {
    type Target = MysqlConnection;

    fn deref(&self) -> &Self::Target {
        match self {
            DbConnectionRef::Simple(conn) => conn,
            DbConnectionRef::Pooled(pooled) => &*pooled,
        }
    }
}

pub fn create_connection_pool(settings: &DatabaseSettings) -> DbConnectionPool {
    let manager = ConnectionManager::<MysqlConnection>::new(&settings.url);
    Pool::builder()
        .max_size(settings.connection_pool_size)
        .max_lifetime(Some(Duration::from_secs(settings.connection_ttl_secs)))
        .connection_timeout(Duration::from_millis(settings.connection_timeout_millis))
        .thread_pool(Arc::new(ScheduledThreadPool::with_name(
            "r2d2-worker-{}",
            settings.thread_pool_size,
        )))
        .build(manager)
        .expect("Failed to create DB connection pool")
}
