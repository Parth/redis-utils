use redis::RedisError;

#[macro_export]
macro_rules! watch {
    ($conn:expr, $keys:expr) => {
        if let Err(e) = deadpool_redis::redis::cmd("WATCH")
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
        if let Err(e) = deadpool_redis::redis::cmd("UNWATCH")
            .query_async::<_, ()>($conn)
            .await
        {
            break Err(DbError(e));
        }
    };
}

/// https://github.com/mitsuhiko/redis-rs/issues/353#issuecomment-666290557
/// an async redis transaction helper
#[macro_export]
macro_rules! tx {
    ($conn:expr, $pipe_name:ident, $keys:expr, $body:expr) => {{
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
                Err(DbError(red_err)) => break Err(DbError(red_err)),
            };

            let tx_success: Option<_> = pipeline.query_async($conn).await.unwrap();

            if let Some(response) = tx_success {
                unwatch!($conn);
                break Ok(response);
            }
        };
        ret
    }};
}

pub enum TxError<T> {
    Abort(T),
    DbError(RedisError),
}

impl<U> From<RedisError> for TxError<U> {
    fn from(err: RedisError) -> Self {
        TxError::DbError(err)
    }
}