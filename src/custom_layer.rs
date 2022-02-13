//! A note to those reading this as an example:
//!
//! This file is largely copied from an internal library and isn't really all that important to the
//! experimentation with Valuable.
//!
//! The only important bit is the addition of `record_value` to the impl of `Visit`.
//!
//! ```no_compile
//! impl<'a> Visit for JsonAttributeVisitor<'a> {
//!     fn record_value(&mut self, field: &tracing::field::Field, value: valuable::Value<'_>) {
//!         self.data_mut().insert(
//!             field.name(),
//!             json!(valuable_serde::Serializable::new(value)),
//!         );
//!     }
//! }
//! ```

use chrono::Utc;
use indexmap::IndexMap;
use serde::{
    ser::{SerializeMap, SerializeSeq},
    Serializer,
};
use serde_json::json;
use std::{cell::Cell, io::Write};
use tracing::{field::Visit, span, Level, Metadata, Subscriber};
use tracing_subscriber::{registry::Scope, Layer};

pub const SPECIAL_JSON_PREFIX: &str = "!custom_layer_tracing_json!";

/// A `tracing_subscriber::Layer` that outputs trace data in a format that we like.
///
/// ```
/// use tracing_subscriber::prelude::*;
/// use tracing_valuable_testing::custom_layer::CustomJsonLayer;
///
/// let layer = CustomJsonLayer::default();
/// tracing_subscriber::registry().with(layer).init();
/// ```
pub struct CustomJsonLayer;

impl Default for CustomJsonLayer {
    fn default() -> Self {
        CustomJsonLayer
    }
}

impl<S> Layer<S> for CustomJsonLayer
where
    S: Subscriber,
    S: for<'lookup> tracing_subscriber::registry::LookupSpan<'lookup>,
{
    fn on_new_span(
        &self,
        attrs: &span::Attributes<'_>,
        id: &span::Id,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        // A new span is created. As far as I can tell, this is the _only_ chance we get to see the
        // attributes for the span. So at this point, we record all of those attributes and store
        // them.
        //
        // The registry context has a convenient place to store this information: as an "extension"
        // on the span itself.

        if let Some(span) = ctx.span(id) {
            let mut data = CustomLayerTracedData::default();
            let mut visitor = JsonAttributeVisitor::with_data(&mut data);
            visitor.record_metadata(span.metadata());
            attrs.record(&mut visitor);

            let mut extensions = span.extensions_mut();
            extensions.insert(data);
        }
    }

    fn on_record(
        &self,
        span: &span::Id,
        values: &span::Record<'_>,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        // New fields are recorded on an existing span.
        //
        // Update the data we've already stored.

        if let Some(span) = ctx.span(span) {
            if let Some(data) = span.extensions_mut().get_mut::<CustomLayerTracedData>() {
                let mut visitor = JsonAttributeVisitor::with_data(data);
                values.record(&mut visitor);
            }
        }
    }

    fn on_event(&self, event: &tracing::Event<'_>, ctx: tracing_subscriber::layer::Context<'_, S>) {
        // An event (created by e.g. `tracing::info!(blah = 3)`) has been created. This is our
        // chance to shine by outputting some JSON to stdout!

        // Convenience: if any of the serialization fails, we want to bail. But we don't want to
        // handle the bail at every location, so we wrap it in a fallible function, and catch the
        // error/bail in one place.
        fn serialize_on_event<S>(
            event: &tracing::Event<'_>,
            ctx: tracing_subscriber::layer::Context<'_, S>,
        ) -> Result<Vec<u8>, serde_json::Error>
        where
            S: Subscriber,
            S: for<'lookup> tracing_subscriber::registry::LookupSpan<'lookup>,
        {
            // Get the data from the event
            let mut data = CustomLayerTracedData::default();
            let mut visitor = JsonAttributeVisitor::with_data(&mut data);
            event.record(&mut visitor);

            // OK, so it would be easier to just build up a big `serde_json::Value` and then output
            // it. However, that would end up with a weird order for the fields. And since these
            // things show up in CloudWatch for us, we kinda want the most important data in the
            // front.
            //
            // So instead we create a `serde_json::Serializer` and serialize a bit more manually.

            let mut serializer = serde_json::Serializer::new(vec![]);
            let mut map_serializer = serializer.serialize_map(None)?;
            // ```
            // {
            //   "timestamp": "2021-04-21T01:02:03.000000001Z",
            //   "level": "INFO",
            //   "target": "word_notifier::module::submodule",
            //   "fields": {
            //     "some_field": "a string",
            //     "another_field": 17
            //   }
            //   ...
            // }
            map_serializer.serialize_entry("timestamp", &json!(Utc::now()))?;
            map_serializer
                .serialize_entry("level", &json!(format_level(event.metadata().level())))?;
            map_serializer.serialize_entry("target", &json!(event.metadata().target()))?;
            map_serializer.serialize_entry("fields", &data)?;

            // If we are in a span, get the closest span and log out it.
            //
            // ```
            // {
            //   ...
            //   "span": {
            //     "target": "zenlist_core::client::actions::get",
            //     "name": "get_option",
            //     "some_field": 1
            //   }
            //   ...
            // }
            // ```
            if let Some(span) = ctx.event_span(event) {
                if let Some(data) = span.extensions().get::<CustomLayerTracedData>() {
                    map_serializer.serialize_entry("span", data)?;
                }
            }

            // Also if we're in a span, get the whole stack of spans we're in and log them
            //
            // ```
            // {
            //   ...
            //   "spans": [
            //     { "target": "...", "name": "...", "some_field": 1 }, // outermost span
            //     { "target": "...", "name": "...", "a_thing": true },
            //     { "target": "...", "name": "...", "different_field": 1, "enabled": true }  // innermost span
            //   ]
            //   ...
            // }
            // ```
            if let Some(scope) = ctx.event_scope(event) {
                let scope_serializer = ScopeSerializer::new(scope);
                map_serializer.serialize_entry("spans", &scope_serializer)?;
            }

            SerializeMap::end(map_serializer)?;
            let mut inner = serializer.into_inner();
            inner.push(b'\n');
            Ok(inner)
        }

        // Create the JSON representation of the event...
        let serialized = match serialize_on_event(event, ctx) {
            Ok(serialized) => serialized,
            Err(_) => return,
        };

        // And write it to stdout!
        let mut stdout = std::io::stdout();
        match stdout.write_all(&serialized) {
            Ok(_) => {}
            Err(_) => return,
        }
        let _ = stdout.flush();
    }
}

/// Visit all event/span data and store it as JSON data.
///
/// By using an `IndexMap`, the data stays in the order that it is specified.
struct JsonAttributeVisitor<'a>(&'a mut CustomLayerTracedData);

impl<'a> JsonAttributeVisitor<'a> {
    /// Create a visitor that inserts into the provided data
    fn with_data(data: &'a mut CustomLayerTracedData) -> Self {
        JsonAttributeVisitor(data)
    }

    /// Get a mutable reference to the interior data
    fn data_mut(&mut self) -> &mut CustomLayerTracedData {
        self.0
    }

    /// Add `target` and `name` to the JSON data that is stored.
    fn record_metadata(&mut self, metadata: &Metadata) {
        let data = self.data_mut();
        data.insert("target", json!(metadata.target()));
        data.insert("name", json!(metadata.name()));
    }
}

impl<'a> Visit for JsonAttributeVisitor<'a> {
    fn record_f64(&mut self, field: &tracing::field::Field, value: f64) {
        self.data_mut().insert(field.name(), json!(value));
    }

    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        self.data_mut().insert(field.name(), json!(value));
    }

    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        self.data_mut().insert(field.name(), json!(value));
    }

    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        self.data_mut().insert(field.name(), json!(value));
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        // If this is a string logged with the special string that represents the JSON hack that
        // we're performing, parse the rest of the string as JSON and use that. Otherwise, it's just
        // a regular string.
        let data = if let Some(json_str) = value.strip_prefix(SPECIAL_JSON_PREFIX) {
            if let Ok(json) = serde_json::from_str(json_str) {
                json
            } else {
                json!(value)
            }
        } else {
            json!(value)
        };
        self.data_mut().insert(field.name(), data);
    }

    fn record_error(
        &mut self,
        field: &tracing::field::Field,
        value: &(dyn std::error::Error + 'static),
    ) {
        self.data_mut()
            .insert(field.name(), json!(value.to_string()));
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.data_mut()
            .insert(field.name(), json!(format!("{:?}", value)));
    }

    fn record_value(&mut self, field: &tracing::field::Field, value: valuable::Value<'_>) {
        self.data_mut().insert(
            field.name(),
            json!(valuable_serde::Serializable::new(value)),
        );
    }
}

/// Data from traced spans that gets stored as extensions inside tracing spans, and can be
/// serialized into the data we want to show.
#[derive(Default)]
struct CustomLayerTracedData(IndexMap<&'static str, serde_json::Value>);

impl CustomLayerTracedData {
    pub fn insert(
        &mut self,
        key: &'static str,
        value: serde_json::Value,
    ) -> Option<serde_json::Value> {
        self.0.insert(key, value)
    }
}

impl serde::Serialize for CustomLayerTracedData {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for (k, v) in &self.0 {
            map.serialize_entry(k, v)?;
        }
        map.end()
    }
}

/// Serialize all of the parent spans of an event as JSON data.
//
// Woah! `Cell<Option<Scope<'a, R>>>`? That's complicated.
//
// It turns out that the scope needs to be _owned_ to get anything valuable out of it. We own it
// when we create this struct, but `serde::Serialize::serialize` is called with a reference. Making
// it a `Cell<Option<..>>` means that when `serde::Serialize::serialize` is called, we can take
// ownership of it so we can serialize what we need to.
struct ScopeSerializer<'a, R: tracing_subscriber::registry::LookupSpan<'a>>(
    Cell<Option<Scope<'a, R>>>,
);

impl<'a, R> ScopeSerializer<'a, R>
where
    R: tracing_subscriber::registry::LookupSpan<'a>,
{
    fn new(v: Scope<'a, R>) -> Self {
        ScopeSerializer(Cell::new(Some(v)))
    }
}

impl<'a, R> serde::Serialize for ScopeSerializer<'a, R>
where
    R: tracing_subscriber::registry::LookupSpan<'a>,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut seq = serializer.serialize_seq(None)?;
        let scope = self.0.replace(None);
        if let Some(scope) = scope {
            for span in scope.from_root() {
                let extensions = span.extensions();
                if let Some(data) = extensions.get::<CustomLayerTracedData>() {
                    seq.serialize_element(data)?;
                }
            }
        }
        seq.end()
    }
}

fn format_level(level: &Level) -> &'static str {
    match *level {
        Level::DEBUG => "DEBUG",
        Level::ERROR => "ERROR",
        Level::INFO => "INFO",
        Level::TRACE => "TRACE",
        Level::WARN => "WARN",
    }
}
