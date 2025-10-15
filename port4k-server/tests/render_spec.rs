use port4k_server::renderer::{
    MissingVarPolicy, RenderOptions, RenderVars, render_template, render_template_with_opts,
};

#[test]
fn var_basic_and_default() {
    let mut vars = RenderVars::default();
    vars.global.insert("name".to_string(), "Nova".to_string());

    assert_eq!(render_template("Hello {v:name}!", &vars, 80), "Hello Nova!");
    assert_eq!(render_template("Hello {v:missing:World}!", &vars, 80), "Hello World!");
}

#[test]
fn var_missing_policy() {
    let vars = RenderVars::default();

    // default: LeaveToken
    assert_eq!(
        render_template("X{v:who}Y", &vars, 80),
        "X\u{1b}[36;41m{{v:who}}\u{1b}[0mY"
    );

    let s = render_template_with_opts(
        "X{v:who}Y",
        &vars,
        &RenderOptions {
            missing_var: MissingVarPolicy::Empty,
            max_width: 80,
        },
    );
    assert_eq!(s, "XY");

    let s = render_template_with_opts(
        "X{v:who}Y",
        &vars,
        &RenderOptions {
            missing_var: MissingVarPolicy::Undefined,
            max_width: 80,
        },
    );
    assert_eq!(s, "XundefinedY");
}

#[test]
fn var_string_format_padding() {
    let mut vars = RenderVars::default();
    vars.global.insert("name".to_string(), "Ada".to_string());

    assert_eq!(render_template("-{v:name|%-6s}-", &vars, 80), "-Ada   -"); // left-align, pad right
    assert_eq!(render_template("-{v:name|%6s}-", &vars, 80), "-   Ada-"); // right-align, pad left
    assert_eq!(render_template("-{v:name|%3s}-", &vars, 80), "-Ada-"); // equal width
    assert_eq!(render_template("-{v:name|%2s}-", &vars, 80), "-Ada-"); // shorter than content
}

#[test]
fn var_int_format_padding_and_zero() {
    let mut vars = RenderVars::default();
    vars.global.insert("score".to_string(), "7".to_string());
    vars.global.insert("big".to_string(), "12345".to_string());

    assert_eq!(render_template("S={v:score|%5d}", &vars, 80), "S=    7");
    assert_eq!(render_template("S={v:score|%05d}", &vars, 80), "S=00007");
    assert_eq!(render_template("B={v:big|%5d}", &vars, 80), "B=12345");
    assert_eq!(render_template("B={v:big|%03d}", &vars, 80), "B=12345"); // width <= len
}

#[test]
fn var_int_parse_fallback_to_zero() {
    let mut vars = RenderVars::default();
    vars.global.insert("weird".to_string(), "abc".to_string());

    // Non-numeric with %d → 0
    assert_eq!(render_template("N={v:weird|%05d}", &vars, 80), "N=00000");
}

#[test]
fn var_default_can_contain_pipes() {
    // Default includes pipes and spaces; fmt applies after default chosen
    let vars = RenderVars::default();
    let s = render_template("-{v:title:alpha | beta | gamma|%25s}-", &vars, 80);
    assert_eq!(s, "-     alpha | beta | gamma-");
}

#[test]
fn color_reset_and_simple_fg() {
    let vars = RenderVars::default();

    let s = render_template("{c:red}X{c}", &vars, 80);
    // Must include an SGR start and reset end
    assert!(s.starts_with("\x1b["));
    assert!(s.contains("[31")); // red fg somewhere
    assert!(s.ends_with("\x1b[0m"));
}

#[test]
fn color_fg_bg_attrs_any_order() {
    let vars = RenderVars::default();

    // yellow on red, bold+underline
    let s = render_template("{c:yellow:red:bold,underline}ALERT{c}", &vars, 80);
    // We don't enforce order; just assert presence
    assert!(s.contains("\x1b[")); // SGR start
    assert!(s.contains("33")); // yellow fg
    assert!(s.contains("41")); // red bg
    assert!(s.contains("1") || s.contains(";1")); // bold
    assert!(s.contains("4") || s.contains(";4")); // underline
    assert!(s.ends_with("\x1b[0m")); // reset
}

#[test]
fn color_attr_second_field_is_attrs_not_bg() {
    let vars = RenderVars::default();

    // second token "bold" is not a color → treated as attrs
    let s = render_template("{c:yellow:bold}Y{c}", &vars, 80);
    // yellow + bold present
    assert!(s.contains("33"));
    assert!(s.contains("1"));
}

#[test]
fn escapes_and_unknown_tokens() {
    let vars = RenderVars::default();

    // Escapes
    assert_eq!(render_template("{{}}", &vars, 80), "{}");
    assert_eq!(
        render_template("{{v}} -> {v:name}", &vars, 80),
        "{v} -> \u{1b}[36;41m{{v:name}}\u{1b}[0m"
    );
    // Unknown token passthrough
    assert_eq!(render_template("X{x:foo}Y", &vars, 80), "X{x:foo}Y");
}

#[test]
fn unterminated_brace_is_literal() {
    let mut vars = RenderVars::default();
    vars.global.insert("name".to_string(), "Nova".to_string());

    assert_eq!(render_template("Hello {v:name", &vars, 80), "Hello {v:name");
}

#[test]
fn multiple_tokens_sequence() {
    let mut vars = RenderVars::default();
    vars.global.insert("a".to_string(), "1".to_string());
    vars.global.insert("b".to_string(), "2".to_string());
    vars.global.insert("c".to_string(), "3".to_string());

    let s = render_template("{v:a}{v:b}{v:c}", &vars, 80);
    assert_eq!(s, "123");
}

#[test]
fn mixed_all_together() {
    let mut vars = RenderVars::default();
    vars.global.insert("pilot".to_string(), "Nova".to_string());
    vars.global.insert("coins".to_string(), "42".to_string());

    let s = render_template(
        "{c:white:blue:bold}Pilot{c}: {v:pilot|%-6s} Coins:{v:coins|%04d}{c}",
        &vars,
        80,
    );
    assert!(s.contains("Pilot"));
    assert!(s.contains("Nova  ")); // left padded to width
    assert!(s.contains("Coins:0042"));
    assert!(s.ends_with("\x1b[0m"));
}
