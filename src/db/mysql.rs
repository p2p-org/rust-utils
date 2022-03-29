use std::ops::Deref;

use diesel::{
    mysql::MysqlConnection,
    r2d2::{ConnectionManager, PooledConnection},
};

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
