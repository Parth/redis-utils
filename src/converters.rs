use crate::TxError;
use redis::{Pipeline, ToRedisArgs};
use serde::Serialize;

pub trait JsonSet<U> {
    fn set_json<Key: ToRedisArgs, Val: Serialize>(
        &mut self,
        key: Key,
        val: Val,
    ) -> Result<&mut Self, TxError<U>>;
}

impl<U> JsonSet<U> for Pipeline {
    fn set_json<Key: ToRedisArgs, Val: Serialize>(
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
