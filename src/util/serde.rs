use serde_json::Value;

/// Convert a serde_json::Value into a string representation suitable for storage. Note that this
/// will always return something, and a empty value on an unsupported type.
pub fn serde_to_str(v: Value) -> String {
    match v {
        Value::String(s) => s,
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => "null".to_string(),
        Value::Array(arr) => {
            // Render each element as JSON (so strings get quotes),
            // then join with ", " and omit the outer brackets.
            let parts: Vec<String> = arr
                .iter()
                .map(|e| serde_json::to_string(e).unwrap_or_else(|_| "null".to_string()))
                .collect();
            parts.join(",")
        }
        _ => "".to_string(),
    }
}
