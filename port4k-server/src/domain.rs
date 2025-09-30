#[allow(unused)]
pub type ObjectId = String;

#[allow(unused)]
pub struct RoomObject {
    pub id: ObjectId,
    pub nouns: Vec<String>,
    pub short: String,
    pub description: String,
    pub examine: Option<String>,
    pub state: serde_json::Value,
    pub use_lua: Option<String>,
    pub position: Option<i32>,
}

impl RoomObject {
    pub fn matches_noun(&self, needle: &str) -> bool {
        let n = needle.trim().to_lowercase();
        self.nouns.iter().any(|x| x.to_lowercase() == n)
    }

    pub fn state_bool(&self, key: &str) -> Option<bool> {
        self.state.get(key).and_then(|v| v.as_bool())
    }
    pub fn with_state_bool(mut self, key: &str, value: bool) -> Self {
        let mut map = self.state.as_object().cloned().unwrap_or_default();
        map.insert(key.to_string(), serde_json::Value::Bool(value));
        self.state = serde_json::Value::Object(map);
        self
    }
}

/// Tiny projection for rendering/linkification
#[derive(Debug, Clone)]
pub struct RenderObject {
    pub id: String,
    pub short: String,
}