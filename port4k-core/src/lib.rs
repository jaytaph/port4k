use std::fmt;


#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Username(pub String);


impl fmt::Display for Username {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { self.0.fmt(f) }
}


impl Username {
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        if s.is_empty() { return None; }
        if !s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' )) {
            return None;
        }
        Some(Self(s.to_string()))
    }
}