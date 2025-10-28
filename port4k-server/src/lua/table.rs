use std::collections::HashSet;

pub fn format_lua_value(value: &mlua::Value) -> String {
    format_lua_value_impl(value, 0, &mut HashSet::new())
}

fn format_lua_value_impl(value: &mlua::Value, indent: usize, seen: &mut HashSet<usize>) -> String {
    match value {
        mlua::Value::Nil => "nil".to_string(),
        mlua::Value::Boolean(b) => b.to_string(),
        mlua::Value::Integer(i) => i.to_string(),
        mlua::Value::Number(n) => n.to_string(),
        mlua::Value::String(s) => format!("\"{}\"", String::from_utf8_lossy(s.as_bytes().as_ref())),
        mlua::Value::Table(t) => print_lua_table(t, indent, seen),
        mlua::Value::Function(_) => "<function>".to_string(),
        mlua::Value::Thread(_) => "<thread>".to_string(),
        mlua::Value::UserData(_) => "<userdata>".to_string(),
        mlua::Value::LightUserData(_) => "<lightuserdata>".to_string(),
        mlua::Value::Error(e) => format!("error: {}", e),
        _ => "<unknown>".to_string(),
    }
}

fn print_lua_table(table: &mlua::Table, indent: usize, seen: &mut HashSet<usize>) -> String {
    // Get a unique identifier for this table to detect cycles
    let table_ptr = table.to_pointer() as usize;

    // Check for circular reference
    if seen.contains(&table_ptr) {
        return "<circular reference>".to_string();
    }

    seen.insert(table_ptr);

    let mut result = String::from("{\n");
    let indent_str = "  ".repeat(indent + 1);

    // Try to get all pairs - if it fails, just show <table>
    let pairs = match table.pairs::<mlua::Value, mlua::Value>().collect::<Result<Vec<_>, _>>() {
        Ok(pairs) => pairs,
        Err(_) => {
            seen.remove(&table_ptr);
            return "<table>".to_string();
        }
    };

    // Separate numeric indices (array part) from other keys
    let mut array_items: Vec<(i64, mlua::Value)> = Vec::new();
    let mut hash_items: Vec<(mlua::Value, mlua::Value)> = Vec::new();

    for (key, value) in pairs {
        if let mlua::Value::Integer(i) = key {
            array_items.push((i, value));
        } else {
            hash_items.push((key, value));
        }
    }

    // Sort array items by index
    array_items.sort_by_key(|(i, _)| *i);

    // Print array part (consecutive integers starting from 1)
    let mut last_idx = 0;
    for (idx, value) in array_items {
        // Check if indices are consecutive
        if idx == last_idx + 1 {
            let formatted_value = format_lua_value_impl(&value, indent + 1, seen);
            result.push_str(&format!("{}{},\n", indent_str, formatted_value));
            last_idx = idx;
        } else {
            // Non-consecutive, treat as hash key
            let formatted_value = format_lua_value_impl(&value, indent + 1, seen);
            result.push_str(&format!("{}[{}] = {},\n", indent_str, idx, formatted_value));
        }
    }

    // Print hash part
    for (key, value) in hash_items {
        let formatted_key = format_lua_key(&key);
        let formatted_value = format_lua_value_impl(&value, indent + 1, seen);
        result.push_str(&format!("{}{} = {},\n", indent_str, formatted_key, formatted_value));
    }

    // Remove trailing comma and newline if present
    if result.ends_with(",\n") {
        result.truncate(result.len() - 2);
        result.push('\n');
    }

    let close_indent = "  ".repeat(indent);
    result.push_str(&format!("{}}}", close_indent));

    seen.remove(&table_ptr);

    result
}

fn format_lua_key(key: &mlua::Value) -> String {
    match key {
        mlua::Value::String(s) => {
            let bytes = s.as_bytes();
            let key_str = String::from_utf8_lossy(bytes.as_ref());
            // Check if it's a valid identifier (no need for brackets)
            if is_valid_lua_identifier(&key_str) {
                key_str.into_owned()
            } else {
                format!("[\"{}\"]", key_str)
            }
        }
        mlua::Value::Integer(i) => format!("[{}]", i),
        mlua::Value::Number(n) => format!("[{}]", n),
        mlua::Value::Boolean(b) => format!("[{}]", b),
        _ => "[<complex key>]".to_string(),
    }
}

fn is_valid_lua_identifier(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }

    // Check if it's a Lua keyword
    const LUA_KEYWORDS: &[&str] = &[
        "and", "break", "do", "else", "elseif", "end", "false", "for", "function", "if", "in", "local", "nil", "not",
        "or", "repeat", "return", "then", "true", "until", "while",
    ];

    if LUA_KEYWORDS.contains(&s) {
        return false;
    }

    // Check if first char is letter or underscore
    let mut chars = s.chars();
    let first = chars.next().unwrap();
    if !first.is_ascii_alphabetic() && first != '_' {
        return false;
    }

    // Check remaining chars are alphanumeric or underscore
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}
