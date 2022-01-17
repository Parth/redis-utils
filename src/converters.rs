use crate::TxError;
use async_trait::async_trait;
use redis::aio::ConnectionLike;
use redis::{AsyncCommands, Pipeline, RedisError, ToRedisArgs};
use serde::de::DeserializeOwned;
use serde::Serialize;

pub trait JsonSet<U> {
    fn json_set<Key: ToRedisArgs, Val: Serialize>(
        &mut self,
        key: Key,
        val: Val,
    ) -> Result<&mut Self, TxError<U>>;
}

impl<U> JsonSet<U> for Pipeline {
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
}

#[async_trait]
impl<C, Val> JsonGet<Val> for C
where
    C: ConnectionLike + Send + Sync,
    Val: DeserializeOwned,
{
    async fn json_get<Key: ToRedisArgs + Send + Sync>(
        &mut self,
        key: Key,
    ) -> Result<Val, JsonGetError> {
        let val: String = self.get(key).await?;
        Ok(serde_json::from_str(&val)?)
    }

    async fn maybe_json_get<Key: ToRedisArgs + Send + Sync>(
        &mut self,
        key: Key,
    ) -> Result<Option<Val>, JsonGetError> {
        let val: Option<String> = self.get(key).await.unwrap();
        Ok(val.map(|string| serde_json::from_str(&string).unwrap()))
    }

    async fn json_mget<Key: ToRedisArgs + Send + Sync>(
        &mut self,
        keys: Key,
    ) -> Result<Vec<Val>, JsonGetError> {
        if keys.is_single_arg() {
            let val: Option<String> = self.get(keys).await.unwrap();
            Ok(val
                .iter()
                .map(|string| serde_json::from_str(string).unwrap())
                .collect())
        } else {
            let val: Vec<String> = self.get(keys).await.unwrap();
            Ok(val
                .iter()
                .map(|string| serde_json::from_str(string).unwrap())
                .collect())
        }
    }
}
