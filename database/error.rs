use thiserror::Error;

use crate::transaction::TryIntoSqlxError;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Unable to execute serializable transaction")]
    SerializationFailure,

    #[error("{0}")]
    StringError(String),

    #[error("SQL Error")]
    SqlError(#[from] sqlx::error::Error),

    #[error("Database Configuration Error: {0}")]
    ConfigError(String),

    #[error("Connection pool closed")]
    PoolClosed,

    #[error("timed out")]
    TimeoutError,

    #[error("Redis connection error {0}")]
    RedisPoolError(#[from] deadpool::managed::PoolError<::redis::RedisError>),

    #[error("Redis pool creation error {0}")]
    RedisPoolCreationError(#[from] deadpool_redis::CreatePoolError),
}

impl sqlx::error::DatabaseError for Error {
    fn message(&self) -> &str {
        match self {
            Error::SqlError(sqlx::Error::Database(e)) => e.message(),
            _ => "",
        }
    }

    fn as_error(&self) -> &(dyn std::error::Error + Send + Sync + 'static) {
        self
    }

    fn as_error_mut(&mut self) -> &mut (dyn std::error::Error + Send + Sync + 'static) {
        self
    }

    fn into_error(self: Box<Self>) -> Box<dyn std::error::Error + Send + Sync + 'static> {
        self
    }
}

impl TryIntoSqlxError for Error {
    fn try_into_sqlx_error(self) -> Result<sqlx::Error, Self> {
        match self {
            Self::SqlError(e) => Ok(e),
            _ => Err(self),
        }
    }
}
