use serde_json::{Value, json};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct ErrorClassification {
    pub category: &'static str,
    pub hint: &'static str,
}

impl ErrorClassification {
    pub(crate) const fn new(category: &'static str, hint: &'static str) -> Self {
        Self { category, hint }
    }

    fn to_value(self) -> Value {
        json!({
            "schemaVersion": 1,
            "category": self.category,
            "hint": self.hint,
        })
    }
}

pub(crate) fn attach_error_classification(
    mut data: Value,
    classification: ErrorClassification,
) -> Value {
    let Value::Object(ref mut object) = data else {
        return json!({
            "schemaVersion": 1,
            "kind": "unknown",
            "value": data,
            "classification": classification.to_value(),
        });
    };

    object.insert("schemaVersion".to_string(), json!(1));
    object.insert("classification".to_string(), classification.to_value());
    data
}
