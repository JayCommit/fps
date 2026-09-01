//! Secret redaction for logs and support bundles.

use fps_observability::looks_secret;
use serde_json::Value;

pub fn redact_json(value: &mut Value) {
    match value {
        Value::Object(map) => {
            let keys: Vec<String> = map.keys().cloned().collect();
            for k in keys {
                if looks_secret(&k) {
                    map.insert(k, Value::String("[redacted]".into()));
                } else if let Some(child) = map.get_mut(&k) {
                    redact_json(child);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                redact_json(item);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn redacts_nested_tokens() {
        let mut v = json!({"node_token": "abc", "hostname": "n1", "nested": {"password": "x"}});
        redact_json(&mut v);
        assert_eq!(v["node_token"], "[redacted]");
        assert_eq!(v["hostname"], "n1");
        assert_eq!(v["nested"]["password"], "[redacted]");
    }
}
