# redis_utils

Cohesive helpers built on top of [redis-rs](https://github.com/mitsuhiko/redis-rs) for:

+ async transactions
+ `get`ing and `set`ing json values

# Async Transactions

A macro that helps you set up a safe async [redis transaction](https://redis.io/topics/transactions).

Takes:

+ A connection
+ The name of a [pipeline](https://docs.rs/redis/latest/redis/struct.Pipeline.html) which it configures
  in [atomic-mode](https://docs.rs/redis/latest/redis/struct.Pipeline.html#method.atomic).
+ A set of keys to `WATCH`
+ The body of the transaction that can get those keys, use the pipeline (for side effects) and if those keys change (and
  the `EXEC` component of the atomic pipeline fails), then the body will be re-executed.
+ Allows for safe early returns (aborted transactions) with typed values, all keys will be un-watched during an early
  return.

```rust
tx!(&mut con, pipe, &["key1"], {
  let mut value: u8 = con.get("key1").await?;
  value = value + 1;
  
  Ok(pipe.set("key1", value))
});
```

## Aborting a tx

```rust
tx!(&mut con, pipe, &["key1"], {
  let mut value: u8 = con.get("key1").await?;
  value = value + 1;
  
  if value == 69 {
    return Err(Abort(BadNumberFound));
  }
  
  Ok(pipe.set("key1", value))
});
```

## Handling return values

```rust
let tx: Result<u8, TxError<NumberError> > = tx!(&mut con, pipe, &["key1"], {
  let mut value: u8 = con.get("key1").await?;
  value = value + 1;
  
  if value == 69 {
    return Err(Abort(BadNumberFound));
  }
  
  Ok(pipe.set("key1", value))
});
```

+ The `Ok(T)` of `tx` is the type that's handed to `pipe.set()` for `redis-rs`'s type inference.
+ `TxError` allows you to return any type in `TxError::Abort` for custom type handling.
+ If the transaction fails due to an underlying `redis` error or `serde` `tx` will reflect this in the
  associated `TxError::DbError` or `TxError::Serialization`. 

# JSON helpers

Using the helpers from [TODO](converters) allow you to turn this:

```rust
let json_string: String = con.get(key).await?;
let value: Type = serde_json::from_str(&json_string).unwrap;
```

```rust
let value: Type = con.json_get(key).await.unwrap();
```