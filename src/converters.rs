use crate::TxError;
use async_trait::async_trait;
use redis::aio::ConnectionLike;
use redis::{AsyncCommands, Pipeline, RedisError, ToRedisArgs};
use serde::de::DeserializeOwned;
use serde::Serialize;

pub trait PipelineJsonSet<U> {
    fn json_set<Key: ToRedisArgs, Val: Serialize>(
        &mut self,
        key: Key,
        val: Val,
    ) -> Result<&mut Self, TxError<U>>;
}

impl<U> PipelineJsonSet<U> for Pipeline {
    fn json_set<Key: ToRedisArgs, Val: Serialize>(
        &mut self,
        key: Key,
        val: Val,
    ) -> Result<&mut Self, TxError<U>> {
        Ok(self.set(
            key,
            serde_json::to_string(&val).map_err(TxError::Serialization)?,
        ))
    }
}

#[derive(Debug)]
pub enum JsonSetError {
    Serialization(serde_json::Error),
    DbError(redis::RedisError),
}

impl From<RedisError> for JsonSetError {
    fn from(err: RedisError) -> Self {
        JsonSetError::DbError(err)
    }
}

impl From<serde_json::Error> for JsonSetError {
    fn from(err: serde_json::Error) -> Self {
        JsonSetError::Serialization(err)
    }
}

#[async_trait]
pub trait JsonSet {
    async fn json_set<Key: ToRedisArgs + Send + Sync, Val: Serialize + Send + Sync>(
        &mut self,
        key: Key,
        val: Val,
    ) -> Result<(), JsonSetError>;
}

#[async_trait]
impl<C> JsonSet for C
where
    C: ConnectionLike + Send + Sync,
{
    async fn json_set<Key: ToRedisArgs + Send + Sync, Val: Serialize + Send + Sync>(
        &mut self,
        key: Key,
        val: Val,
    ) -> Result<(), JsonSetError> {
        Ok(self.set(key, serde_json::to_string(&val)?).await?)
    }
}

#[derive(Debug)]
pub enum JsonGetError {
    Serialization(serde_json::Error),
    DbError(redis::RedisError),
}

impl From<RedisError> for JsonGetError {
    fn from(err: RedisError) -> Self {
        JsonGetError::DbError(err)
    }
}

impl From<serde_json::Error> for JsonGetError {
    fn from(err: serde_json::Error) -> Self {
        JsonGetError::Serialization(err)
    }
}

#[async_trait]
pub trait JsonGet<Val> {
    async fn json_get<Key: ToRedisArgs + Send + Sync>(
        &mut self,
        key: Key,
    ) -> Result<Val, JsonGetError>;
    async fn maybe_json_get<Key: ToRedisArgs + Send + Sync>(
        &mut self,
        key: Key,
    ) -> Result<Option<Val>, JsonGetError>;
    async fn json_mget<Key: ToRedisArgs + Send + Sync>(
        &mut self,
        key: Key,
    ) -> Result<Vec<Val>, JsonGetError>;
    async fn watch_json_mget<Key: ToRedisArgs + Send + Sync>(
        &mut self,
        key: Key,
    ) -> Result<Vec<Val>, JsonGetError>;
}

/// ```no_run
/// extern crate redis_utils;
/// extern crate redis;
///
/// use serde::Deserialize;
/// use redis::{RedisResult, AsyncCommands};
/// use redis_utils::converters::JsonGet;
///
/// #[derive(Deserialize)]
/// struct Person {
///     name: String,
///     age: u8,
/// }
///
/// async fn json_demo() -> RedisResult<()> {
///     let mut con = redis::Client::open("redis://127.0.0.1/")?.get_async_connection().await?;
///     let a: Person = con.json_get("key").await.unwrap();
///
///     Ok(())
/// }
/// ```
#[async_trait]
impl<C, Val> JsonGet<Val> for C
where
    C: ConnectionLike + Send + Sync,
    Val: DeserializeOwned,
{
    /// get -> deserialize it from json
    async fn json_get<Key: ToRedisArgs + Send + Sync>(
        &mut self,
        key: Key,
    ) -> Result<Val, JsonGetError> {
        let val: String = self.get(key).await?;
        Ok(serde_json::from_str(&val)?)
    }

    /// get -> deserialize it from json into an optional value
    async fn maybe_json_get<Key: ToRedisArgs + Send + Sync>(
        &mut self,
        key: Key,
    ) -> Result<Option<Val>, JsonGetError> {
        let val: Option<String> = self.get(key).await?;
        match val {
            Some(string) => Ok(Some(serde_json::from_str(&string)?)),
            None => Ok(None),
        }
    }

    /// mget -> deserialize it from json into a vector of values
    async fn json_mget<Key: ToRedisArgs + Send + Sync>(
        &mut self,
        keys: Key,
    ) -> Result<Vec<Val>, JsonGetError> {
        if keys.is_single_arg() {
            Ok(self.maybe_json_get(keys).await?.into_iter().collect())
        } else {
            let strings: Vec<String> = self.get(keys).await?;
            let mut values = vec![];
            for string in strings {
                values.push(serde_json::from_str(&string)?)
            }
            Ok(values)
        }
    }

    /// watch -> mget -> deserialize it from json into a vector of values
    async fn watch_json_mget<Key: ToRedisArgs + Send + Sync>(
        &mut self,
        key: Key,
    ) -> Result<Vec<Val>, JsonGetError> {
        redis::cmd("WATCH").arg(&key).query_async(self).await?;
        Ok(self.json_mget(key).await?)
    }
}
