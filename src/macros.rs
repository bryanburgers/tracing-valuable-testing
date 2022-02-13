use crate::serde_json_adapter::SerdeJsonAdapter;

/// Logs a serializable object in a tracing context.
///
/// # Examples
///
/// ```
/// use tracing_valuable_testing::macros::tracing_json_new;
///
/// # fn main() {
/// # #[derive(serde::Serialize)]
/// # struct Position { x: f32, y: f32 }
/// let pos = Position { x: 3.234, y: -1.223 };
///
/// tracing::info!(some_field = 3, position = tracing_json_old!(pos));
/// # }
/// ```
#[macro_export]
macro_rules! tracing_json_old_macro {
    ($e:expr) => {
        $crate::macros::tracing_json_old_helper(&$e).as_str()
    };
}

/// Logs a serializable object in a tracing context.
///
/// # Examples
///
/// ```
/// use tracing_valuable_testing::macros::tracing_json_new;
///
/// # fn main() {
/// # #[derive(serde::Serialize)]
/// # struct Position { x: f32, y: f32 }
/// let pos = Position { x: 3.234, y: -1.223 };
///
/// tracing::info!(some_field = 3, position = tracing_json_new!(pos));
/// # }
/// ```
#[macro_export]
macro_rules! tracing_json_new_macro {
    ($e:expr) => {
        $crate::macros::tracing_json_new_helper(&$e).as_value()
    };
}

/// A very hacky way to include rich JSON data in our tracing data.
///
/// The idea here is that, given a serializable structure that we want to log, we can serialize it
/// and put it in the event as a string. We add a prefix to it so that our layer can parse it out
/// later.
///
/// So for example
///
/// ```
/// # use tracing_valuable_testing::macros::tracing_json_old;
/// #[derive(serde::Serialize)]
/// struct Wizard {
///   name: String,
///   age: usize,    
/// };
///
/// let wizard = Wizard { name: String::from("Gandalf"), age: 2000 };
/// tracing::info!(id = 100, wizard = tracing_json_old!(wizard))
/// ```
///
/// is actually equivalent to
///
/// ```
/// tracing::info!(id = 100, wizard = r#"!custom_layer_tracing_json!{"name":"Gandalf","age":2000}"#);
/// ```
///
/// The layer then recognizes the prefix `"!custom_layer_tracing_json!"` and parses the rest of the string
/// as JSON.
pub fn tracing_json_old_helper<S>(value: &S) -> String
where
    S: serde::Serialize,
{
    format!(
        "{}{}",
        crate::custom_layer::SPECIAL_JSON_PREFIX,
        serde_json::to_string(value).unwrap()
    )
}

/// A less hacky way to include rich JSON data in our tracing data, using Valuable
pub fn tracing_json_new_helper<S>(value: &S) -> SerdeJsonAdapter<serde_json::Value>
where
    S: serde::Serialize,
{
    let json = serde_json::to_value(value).unwrap();
    SerdeJsonAdapter::new(json)
}

pub(crate) use tracing_json_new_macro as tracing_json_new;
pub(crate) use tracing_json_old_macro as tracing_json_old;
