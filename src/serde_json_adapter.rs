use valuable::{Listable, Mappable, Valuable, Value};

/// An adapter that exposes a `serde_json::Value` as a `valuable::Valuable`.
///
/// The type parameter exists only to handle both the owned `serde_json::Value` and the borrowed
/// `&serde_json::Value` cases with the same struct.
pub struct SerdeJsonAdapter<T>(T);

impl<T> SerdeJsonAdapter<T>
where
    T: std::borrow::Borrow<serde_json::Value>,
{
    /// Wrap a `serde_json::Value` in the adapter so it can treated as a `valuable::Valuable`.
    pub fn new(t: T) -> Self {
        Self(t)
    }
}

impl<T> Valuable for SerdeJsonAdapter<T>
where
    T: std::borrow::Borrow<serde_json::Value>,
{
    fn as_value(&self) -> Value<'_> {
        match self.0.borrow() {
            serde_json::Value::Null => Value::Unit,
            serde_json::Value::Bool(b) => Value::Bool(*b),
            serde_json::Value::Number(n) => {
                if let Some(u64) = n.as_u64() {
                    Value::U64(u64)
                } else if let Some(i64) = n.as_i64() {
                    Value::I64(i64)
                } else if let Some(f64) = n.as_f64() {
                    Value::F64(f64)
                } else {
                    Value::Unit
                }
            }
            serde_json::Value::String(s) => Value::String(s.as_str()),
            serde_json::Value::Array(_) => Value::Listable(self),
            serde_json::Value::Object(_) => Value::Mappable(self),
        }
    }

    fn visit(&self, visit: &mut dyn valuable::Visit) {
        match self.0.borrow() {
            serde_json::Value::Array(arr) => {
                for item in arr {
                    visit.visit_value(SerdeJsonAdapter(item).as_value());
                }
            }
            serde_json::Value::Object(obj) => {
                for (key, value) in obj {
                    visit.visit_entry(key.as_str().as_value(), SerdeJsonAdapter(value).as_value());
                }
            }
            _ => visit.visit_value(self.as_value()),
        }
    }
}

impl<T> Listable for SerdeJsonAdapter<T>
where
    T: std::borrow::Borrow<serde_json::Value>,
{
    fn size_hint(&self) -> (usize, Option<usize>) {
        if let Some(arr) = self.0.borrow().as_array() {
            (arr.len(), Some(arr.len()))
        } else {
            (0, None)
        }
    }
}

impl<T> Mappable for SerdeJsonAdapter<T>
where
    T: std::borrow::Borrow<serde_json::Value>,
{
    fn size_hint(&self) -> (usize, Option<usize>) {
        if let Some(obj) = self.0.borrow().as_object() {
            (obj.len(), Some(obj.len()))
        } else {
            (0, None)
        }
    }
}
