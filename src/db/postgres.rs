use diesel::{
    pg::PgConnection,
    r2d2::{ConnectionManager, PooledConnection},
};

use std::ops::Deref;

pub enum DbConnectionRef<'a> {
    Simple(&'a PgConnection),
    Pooled(PooledConnection<ConnectionManager<PgConnection>>),
}

impl Deref for DbConnectionRef<'_> {
    type Target = PgConnection;

    fn deref(&self) -> &Self::Target {
        match self {
            DbConnectionRef::Simple(conn) => conn,
            DbConnectionRef::Pooled(pooled) => &*pooled,
        }
    }
}
