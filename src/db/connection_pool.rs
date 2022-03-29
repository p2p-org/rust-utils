use std::{sync::Arc, time::Duration};

use diesel::r2d2::{ConnectionManager, ManageConnection, Pool};
use scheduled_thread_pool::ScheduledThreadPool;
use serde::Deserialize;

pub type DbConnectionPool<T> = Pool<ConnectionManager<T>>;

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

pub enum DbConnection<T>
where
    T: diesel::Connection + ManageConnection,
{
    Simple(T),
    Pooled(DbConnectionPool<T>),
}

pub fn create_connection_pool<T>(settings: &DatabaseSettings) -> DbConnectionPool<T>
where
    T: diesel::Connection + ManageConnection,
{
    let manager = ConnectionManager::<T>::new(&settings.url);
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
