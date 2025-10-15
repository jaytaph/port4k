use std::collections::HashMap;
use port4k_server::renderer::{render_template, render_template_with_opts, RenderOptions, MissingVarPolicy};

fn vars(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

#[test]
fn var_basic_and_default() {
    let v = vars(&[("name", "Nova")]);
    assert_eq!(render_template("Hello {v:name}!", &v), "Hello Nova!");
    assert_eq!(render_template("Hello {v:missing:World}!", &v), "Hello World!");
}

#[test]
fn var_missing_policy() {
    let v = HashMap::new();
    // default: LeaveToken
    assert_eq!(render_template("X{v:who}Y", &v), "X{v:who}Y");

    let s = render_template_with_opts("X{v:who}Y", &v, &RenderOptions { missing_var: MissingVarPolicy::Empty });
    assert_eq!(s, "XY");

    let s = render_template_with_opts("X{v:who}Y", &v, &RenderOptions { missing_var: MissingVarPolicy::Undefined });
    assert_eq!(s, "XundefinedY");
}

#[test]
fn var_string_format_padding() {
    let v = vars(&[("name", "Ada")]);
    assert_eq!(render_template("-{v:name|%-6s}-", &v), "-Ada   -"); // left-align, pad right
    assert_eq!(render_template("-{v:name|%6s}-", &v), "-   Ada-"); // right-align, pad left
    assert_eq!(render_template("-{v:name|%3s}-", &v), "-Ada-");    // equal width
    assert_eq!(render_template("-{v:name|%2s}-", &v), "-Ada-");    // shorter than content
}

#[test]
fn var_int_format_padding_and_zero() {
    let v = vars(&[("score", "7"), ("big", "12345")]);
    assert_eq!(render_template("S={v:score|%5d}", &v), "S=    7");
    assert_eq!(render_template("S={v:score|%05d}", &v), "S=00007");
    assert_eq!(render_template("B={v:big|%5d}", &v), "B=12345");
    assert_eq!(render_template("B={v:big|%03d}", &v), "B=12345"); // width <= len
}

#[test]
fn var_int_parse_fallback_to_zero() {
    let v = vars(&[("weird", "abc")]);
    // Non-numeric with %d → 0
    assert_eq!(render_template("N={v:weird|%05d}", &v), "N=00000");
}

#[test]
fn var_default_can_contain_pipes() {
    // Default includes pipes and spaces; fmt applies after default chosen
    let v = HashMap::new();
    let s = render_template("-{v:title:alpha | beta | gamma|%25s}-", &v);
    assert_eq!(s, "-     alpha | beta | gamma-");
}

#[test]
fn color_reset_and_simple_fg() {
    let v = HashMap::new();
    let s = render_template("{c:red}X{c}", &v);
    // Must include an SGR start and reset end
    assert!(s.starts_with("\x1b["));
    assert!(s.contains("[31")); // red fg somewhere
    assert!(s.ends_with("\x1b[0m"));
}

#[test]
fn color_fg_bg_attrs_any_order() {
    let v = HashMap::new();
    // yellow on red, bold+underline
    let s = render_template("{c:yellow:red:bold,underline}ALERT{c}", &v);
    // We don't enforce order; just assert presence
    assert!(s.contains("\x1b["));     // SGR start
    assert!(s.contains("33"));        // yellow fg
    assert!(s.contains("41"));        // red bg
    assert!(s.contains("1") || s.contains(";1")); // bold
    assert!(s.contains("4") || s.contains(";4")); // underline
    assert!(s.ends_with("\x1b[0m")); // reset
}

#[test]
fn color_attr_second_field_is_attrs_not_bg() {
    let v = HashMap::new();
    // second token "bold" is not a color → treated as attrs
    let s = render_template("{c:yellow:bold}Y{c}", &v);
    // yellow + bold present
    assert!(s.contains("33"));
    assert!(s.contains("1"));
}

#[test]
fn escapes_and_unknown_tokens() {
    let v = HashMap::new();
    // Escapes
    assert_eq!(render_template("{{}}", &v), "{}");
    assert_eq!(render_template("{{v}} -> {v:name}", &v), "{v} -> {v:name}");
    // Unknown token passthrough
    assert_eq!(render_template("X{x:foo}Y", &v), "X{x:foo}Y");
}

#[test]
fn unterminated_brace_is_literal() {
    let v = vars(&[("name", "Nova")]);
    assert_eq!(render_template("Hello {v:name", &v), "Hello {v:name");
}

#[test]
fn multiple_tokens_sequence() {
    let v = vars(&[("a","1"),("b","2"),("c","3")]);
    let s = render_template("{v:a}{v:b}{v:c}", &v);
    assert_eq!(s, "123");
}

#[test]
fn mixed_all_together() {
    let v = vars(&[("pilot","Nova"),("coins","42")]);
    let s = render_template(
        "{c:white:blue:bold}Pilot{c}: {v:pilot|%-6s} Coins:{v:coins|%04d}{c}",
        &v
    );
    assert!(s.contains("Pilot"));
    assert!(s.contains("Nova  ")); // left padded to width
    assert!(s.contains("Coins:0042"));
    assert!(s.ends_with("\x1b[0m"));
}
