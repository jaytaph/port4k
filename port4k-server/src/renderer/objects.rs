use once_cell::sync::Lazy;
use regex::Regex;
use crate::models::room::RoomObject;

// Adjust these signatures to your actual types
struct RenderVars<'a> {
    // ... existing fields
    pub room_objects: &'a [RoomObject], // Vec from RoomView is fine
}

impl<'a> RenderVars<'a> {
    fn find_object(&self, key: &str) -> Option<&RoomObject> {
        // Match by `key` first; fall back to `name` if that’s what you store
        self.room_objects.iter().find(|o| o.name.eq_ignore_ascii_case(key))
    }
}

static O_RE: Lazy<Regex> = Lazy::new(|| {
    // {o:key}, {o:key.field}, {o:key|default}, {o:key.field|default}
    Regex::new(r"\{o:(?P<key>[a-zA-Z0-9_\-]+)(?:\.(?P<field>[a-zA-Z0-9_\-]+))?(?:\|(?P<default>[^}]*))?\}")
        .expect("valid regex")
});

fn expand_inline_object_tokens(input: &str, vars: &RenderVars) -> String {
    O_RE.replace_all(input, |caps: &regex::Captures| {
        let key = &caps["key"];
        let field = caps.name("field").map(|m| m.as_str());
        let default = caps.name("default").map(|m| m.as_str()).unwrap_or("");

        match vars.find_object(key).filter(|o| o.is_visible()) {
            Some(obj) => render_obj_field(obj, field),
            None => default.to_string(), // not found or not visible → use default (or empty)
        }
    }).into_owned()
}

fn render_obj_field(obj: &RoomObject, field: Option<&str>) -> String {
    match field.unwrap_or("short").to_ascii_lowercase().as_str() {
        "short" => obj.short.clone(),
        "name"  => obj.name.clone(),
        // Add more when you expose them, keep simple for now:
        // "desc" | "description" => obj.description.clone().unwrap_or_default(),
        _ => obj.short.clone(), // fallback to short
    }
}