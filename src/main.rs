use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::info;
use tracing_subscriber::prelude::*;
use valuable::Valuable;

mod custom_layer;
mod macros;
mod serde_json_adapter;

use macros::{tracing_json_new, tracing_json_old};
use serde_json_adapter::SerdeJsonAdapter;

fn main() {
    {
        let _default = tracing_subscriber::registry()
            .with(tracing_subscriber::fmt::layer().compact())
            .set_default();
        log_some_things();
    }

    {
        let _default = tracing_subscriber::registry()
            .with(tracing_subscriber::fmt::layer().json())
            .set_default();
        log_some_things();
    }

    {
        let _default = tracing_subscriber::registry()
            .with(custom_layer::CustomJsonLayer::default())
            .set_default();
        log_some_things();
    }
}

fn log_some_things() {
    let serialize_and_valuable = SerializeAndValuable {
        name: String::from("One"),
        aliases: vec![String::from("Two"), String::from("Three")],
    };

    info!(
        message = "as_valuable",
        serialize_and_valuable = serialize_and_valuable.as_value()
    );

    info!(
        message = "as_serialize",
        serialize_and_valuable =
            SerdeJsonAdapter::new(serde_json::to_value(serialize_and_valuable).unwrap()).as_value()
    );

    for json in [
        json!(true),
        json!("hey"),
        json!(42_i32),
        json!(null),
        json!({
            "name": "One",
            "aliases": ["Two", "Three"],
            "age": 37.5_f64,
            "null": null,
        }),
    ] {
        info!(
            message = "adapter test",
            json = SerdeJsonAdapter::new(json).as_value()
        );
    }

    let fancy = FancySerialization {
        fancy_id: String::from("WOO!"),
        fizz_buzz: 15.into(),
        the_rest: BTreeMap::from([
            (String::from("one"), String::from("1")),
            (String::from("two"), String::from("2")),
            (String::from("three"), String::from("3")),
        ]),
    };
    info!(
        message = "fancy",
        json_old = tracing_json_old!(fancy),
        json_new = tracing_json_new!(fancy),
    );
}

#[derive(Debug, Serialize, Deserialize, Valuable)]
struct SerializeAndValuable {
    name: String,
    aliases: Vec<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct FancySerialization {
    #[serde(rename = "fancyId")]
    fancy_id: String,

    #[serde(skip_serializing_if = "FizzBuzz::is_fizz_buzz")]
    fizz_buzz: FizzBuzz,

    #[serde(flatten)]
    the_rest: BTreeMap<String, String>,
}

#[derive(Debug, serde::Serialize)]
#[serde(transparent)]
pub struct FizzBuzz(usize);

impl From<usize> for FizzBuzz {
    fn from(val: usize) -> Self {
        Self(val)
    }
}

impl FizzBuzz {
    pub fn is_fizz(&self) -> bool {
        self.0 % 3 == 0
    }
    pub fn is_buzz(&self) -> bool {
        self.0 % 5 == 0
    }
    pub fn is_fizz_buzz(&self) -> bool {
        self.is_fizz() && self.is_buzz()
    }
}
