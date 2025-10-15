pub const RESET: &str = "\x1b[0m";

/// Build an ANSI SGR sequence given fg/bg names and attributes.
/// Unknown names are ignored; if nothing maps, returns "".
pub fn compose_sgr(fg: Option<&str>, bg: Option<&str>, attrs: &[String]) -> String {
    let mut codes: Vec<&'static str> = Vec::new();

    if let Some(name) = fg
        && let Some(code) = fg_code(name)
    {
        codes.push(code);
    }
    if let Some(name) = bg
        && let Some(code) = bg_code(name)
    {
        codes.push(code);
    }
    for a in attrs {
        if let Some(code) = attr_code(a.as_str()) {
            codes.push(code);
        }
    }

    if codes.is_empty() {
        return String::new();
    }

    let mut s = String::from("\x1b[");
    for (i, c) in codes.iter().enumerate() {
        if i > 0 {
            s.push(';');
        }
        s.push_str(c);
    }
    s.push('m');
    s
}

fn fg_code(name: &str) -> Option<&'static str> {
    match norm(name).as_str() {
        "black" => Some("30"),
        "red" => Some("31"),
        "green" => Some("32"),
        "yellow" => Some("33"),
        "blue" => Some("34"),
        "magenta" => Some("35"),
        "cyan" => Some("36"),
        "white" => Some("37"),
        "gray" | "grey" => Some("90"),
        "bright_black" => Some("90"),
        "bright_red" => Some("91"),
        "bright_green" => Some("92"),
        "bright_yellow" => Some("93"),
        "bright_blue" => Some("94"),
        "bright_magenta" => Some("95"),
        "bright_cyan" => Some("96"),
        "bright_white" => Some("97"),
        "default" | "reset" => Some("39"),
        _ => None,
    }
}

fn bg_code(name: &str) -> Option<&'static str> {
    match norm(name).as_str() {
        "black" => Some("40"),
        "red" => Some("41"),
        "green" => Some("42"),
        "yellow" => Some("43"),
        "blue" => Some("44"),
        "magenta" => Some("45"),
        "cyan" => Some("46"),
        "white" => Some("47"),
        "gray" | "grey" => Some("100"),
        "bright_black" => Some("100"),
        "bright_red" => Some("101"),
        "bright_green" => Some("102"),
        "bright_yellow" => Some("103"),
        "bright_blue" => Some("104"),
        "bright_magenta" => Some("105"),
        "bright_cyan" => Some("106"),
        "bright_white" => Some("107"),
        "default" | "reset" => Some("49"),
        _ => None,
    }
}

fn attr_code(name: &str) -> Option<&'static str> {
    match norm(name).as_str() {
        "bold" => Some("1"),
        "dim" => Some("2"),
        "italic" => Some("3"),
        "underline" => Some("4"),
        "blink" => Some("5"),
        "inverse" | "reverse" => Some("7"),
        _ => None,
    }
}

fn norm(s: &str) -> String {
    s.trim().to_ascii_lowercase()
}
