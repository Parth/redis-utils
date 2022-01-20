//! # redis_utils
//!
//! Cohesive helpers built on top of [redis-rs](https://github.com/mitsuhiko/redis-rs) for:
//!
//! + async transactions
//! + `get`ing and `set`ing json values
//!
//! # Async Transactions
//!
//! A macro that helps you set up a safe async [redis transaction](https://redis.io/topics/transactions).
//!
//! Takes:
//!
//! + A connection
//! + The name of a [pipeline](https://docs.rs/redis/latest/redis/struct.Pipeline.html) which it configures
//! in [atomic-mode](https://docs.rs/redis/latest/redis/struct.Pipeline.html#method.atomic).
//! + A set of keys to `WATCH`
//! + The body of the transaction that can get those keys, use the pipeline (for side effects) and if those keys change (and
//! the `EXEC` component of the atomic pipeline fails), then the body will be re-executed.
//! + Allows for safe early returns (aborted transactions) with typed values, all keys will be un-watched during an early
//! return.
//!
//! ```rust
//! tx!(&mut con, pipe, &["key1"], {
//!   let mut value: u8 = con.get("key1").await?;
//!   value = value + 1;
//!
//!   Ok(pipe.set("key1", value))
//! });
//! ```
//!
//! ## Aborting a tx
//!
//! ```rust
//! tx!(&mut con, pipe, &["key1"], {
//!   let mut value: u8 = con.get("key1").await?;
//!   value = value + 1;
//!
//!   if value == 69 {
//!     return Err(Abort(BadNumberFound));
//!   }
//!
//!   Ok(pipe.set("key1", value))
//! });
//! ```
//!
//! ## Handling return values
//!
//! ```rust
//! let tx: Result<u8, TxError<NumberError> > = tx!(&mut con, pipe, &["key1"], {
//!   let mut value: u8 = con.get("key1").await?;
//!   value = value + 1;
//!
//!   if value == 69 {
//!     return Err(Abort(BadNumberFound));
//!   }
//!
//!   Ok(pipe.set("key1", value))
//! });
//! ```
//!
//! + The `Ok(T)` of `tx` is the type that's handed to `pipe.set()` for `redis-rs`'s type inference.
//! + `TxError` allows you to return any type in `TxError::Abort` for custom type handling.
//! + If the transaction fails due to an underlying `redis` error or `serde` `tx` will reflect this in the
//! associated `TxError::DbError` or `TxError::Serialization`.
//!
//! # JSON helpers
//!
//! Using the helpers from [TODO](converters) allow you to turn this:
//!
//! ```rust
//! let json_string: String = con.get(key).await?;
//! let value: Type = serde_json::from_str(&json_string).unwrap;
//!  ```
//!
//! ```rust
//! let value: Type = con.json_get(key).await.unwrap();
//! ```
//!

use crate::converters::JsonGetError;

pub mod converters;

#[macro_export]
macro_rules! watch {
    ($conn:expr, $keys:expr) => {
        if let Err(e) = redis::cmd("WATCH")
            .arg($keys)
            .query_async::<_, ()>($conn)
            .await
        {
            break Err(DbError(e));
        }
    };
}

#[macro_export]
macro_rules! unwatch {
    ($conn:expr) => {
        if let Err(e) = redis::cmd("UNWATCH").query_async::<_, ()>($conn).await {
            break Err(DbError(e));
        }
    };
}

/// # Async Transactions
///
/// A macro that helps you set up a safe async [redis transaction](https://redis.io/topics/transactions).
///
/// Takes:
///
/// + A connection
/// + The name of a [pipeline](https://docs.rs/redis/latest/redis/struct.Pipeline.html) which it configures
/// in [atomic-mode](https://docs.rs/redis/latest/redis/struct.Pipeline.html#method.atomic).
/// + A set of keys to `WATCH`
/// + The body of the transaction that can get those keys, use the pipeline (for side effects) and if those keys change (and
/// the `EXEC` component of the atomic pipeline fails), then the body will be re-executed.
/// + Allows for safe early returns (aborted transactions) with typed values, all keys will be un-watched during an early
/// return.
///
///```no_run
/// #[macro_use] extern crate redis_utils;
/// extern crate redis;
///
/// use redis::{RedisResult, AsyncCommands};
/// use redis_utils::TxError;
///
/// async fn tx_demo() -> RedisResult<()> {
///     let mut con = redis::Client::open("redis://127.0.0.1/")?.get_async_connection().await?;
///     let tx_result: Result<u8, TxError<()>> = tx!(&mut con, pipe, &["key1"], {
///       let mut value: u8 = con.get("key1").await?;
///       value = value + 1;
///
///       Ok(pipe.set("key1", value))
///     });
///
///    Ok(())
/// }
/// ```
///
/// ## Aborting a tx
///
///```no_run
/// #[macro_use] extern crate redis_utils;
/// extern crate redis;
///
/// use redis::{RedisResult, AsyncCommands};
/// use redis_utils::TxError;
///
/// async fn tx_demo() -> RedisResult<()> {
///     let mut con = redis::Client::open("redis://127.0.0.1/")?.get_async_connection().await?;
///     let tx_result: Result<u8, TxError<&str>> = tx!(&mut con, pipe, &["key1"], {
///       let mut value: u8 = con.get("key1").await?;
///       value = value + 1;
///
///       if value == 69 {
///         return Err(Abort("BadNumberFound"));
///       }
///
///       Ok(pipe.set("key1", value))
///     });
///
///    Ok(())
/// }
/// ```
///
/// + The `Ok(T)` of `tx` is the type that's handed to `pipe.set()` for `redis-rs`'s type inference.
/// + `TxError` allows you to return any type in `TxError::Abort` for custom type handling.
/// + If the transaction fails due to an underlying `redis` error or `serde` `tx` will reflect this in the
/// associated `TxError::DbError` or `TxError::Serialization`.
///
#[macro_export]
macro_rules! tx {
    ($conn:expr, $pipe_name:ident, $keys:expr, $body:expr) => {{
        use redis::pipe;
        use redis::Pipeline;
        use redis_utils::TxError;
        use redis_utils::TxError::{Abort, DbError, Serialization};
        use redis_utils::{unwatch, watch};

        let ret: Result<_, TxError<_>> = loop {
            watch!($conn, $keys);

            let mut $pipe_name = pipe();
            $pipe_name.atomic();

            let create_tx: Result<&mut Pipeline, TxError<_>> = async { $body }.await;

            let pipeline: &mut Pipeline = match create_tx {
                Ok(pipeline) => pipeline,
                Err(Abort(value)) => {
                    unwatch!($conn);
                    break Err(Abort(value));
                }
                Err(Serialization(value)) => {
                    unwatch!($conn);
                    break Err(Serialization(value));
                }
                Err(DbError(red_err)) => break Err(DbError(red_err)),
            };

            let tx_success: Option<_> = pipeline.query_async($conn).await?;

            if let Some(response) = tx_success {
                unwatch!($conn);
                break Ok(response);
            }
        };
        ret
    }};
}

/// Represents the various ways a transaction can return early. It could be `Abort`ed early due to
/// some precondition failure. It could fail due to a `Serialization` error if you're using any of
/// the `redis_utils::converters`. Or if there is an underlying `RedisError`.
pub enum TxError<T> {
    Abort(T),
    Serialization(serde_json::Error),
    DbError(redis::RedisError),
}

impl<U> From<JsonGetError> for TxError<U> {
    fn from(err: JsonGetError) -> Self {
        match err {
            JsonGetError::Serialization(err) => TxError::Serialization(err),
            JsonGetError::DbError(err) => TxError::DbError(err),
        }
    }
}

impl<U> From<redis::RedisError> for TxError<U> {
    fn from(err: redis::RedisError) -> Self {
        TxError::DbError(err)
    }
}
