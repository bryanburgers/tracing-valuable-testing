Some playing around with `tracing`'s new support for `valuable`.

Based on [*Announcing Experimental `valuable` Support*](https://github.com/tokio-rs/tracing/discussions/1906), this repo evaluates whether we can replace our custom structured logging method with Valuable.

Our current library does structured JSON logging using a custom tracing layer. The way we get structured logging is by using a macro that serializes `serde::Serialize` objects to a string with a specific prefix, then the tracing layer looks for that prefix and deserializes it back into JSON.

Using `valuable::Valuable`, we could instead use an adapter to treate a `serde::Serialize` as a thing that implements `valuable::Valuable`, and then in the tracing layer conver that `valuable::Valuable` back to a `serde_json::Value`.

Either way, we will probably continue to end up with code that looks like the following when we want to log an entire model. What `tracing_json!` does under-the-hood would change.

```rust
info!(message = "received model", model = tracing_json!(model));
```

Parts of this example:

* `serde_json_adapter::SerdeJsonAdapter` – An adapter to expose `serde_json::Value` as a `valuable::Valuable`. Ideally this would expose an adapter for any `serde::Serialize`, but for testing purposes this was sufficient.
* `macros::{tracing_json_old, tracing_json_new}` – The macros that represent `tracing_json!` in the example above. `_old` converts to a string like `"!custom_prefix!{\"id\":\"abc\"}"`; `_new` converts to something like `SerdeJsonAdapter::new(json!({"id": "abc"})).as_value()`.
* `custom_layer.rs` – Our custom logging layer, copied from our core library. The details are largely unimportant, but `tracing_subscriber::fmt::layer().json()` doesn't currently seem to convert `valuable::Valuable` into json, so I needed something that would.
