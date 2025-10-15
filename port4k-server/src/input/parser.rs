//! Command parser for MUD-style inputs.
//!
//! Examples:
//!   "open the door"              -> Verb::Open, direct="door"
//!   "open door with key"         -> Verb::Open, direct="door", preposition=With, instrument="key"
//!   "look at markings"           -> Verb::Look, preposition=At, direct="markings"
//!   "put coin into toolkit"      -> Verb::Put, direct="coin", preposition=In, target="toolkit"
//!   "take all coins from bag"    -> Verb::Take, quantifier=All, direct="coins", preposition=From, target="bag"
//!   "n" or "go north"            -> Verb::Go, direction=North
//!   "pick up screwdriver"        -> Verb::Take, direct="screwdriver"
//!
//! Usage:
//!   let intent = parse_command("open the door with key");
//!   match intent.verb { Verb::Open => { /* inspect intent.direct/instrument */ }, _ => {} }

use crate::models::types::Direction;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verb {
    Look,
    Examine,
    Search,
    Take,
    Drop,
    Open,
    Close,
    Unlock,
    Lock,
    Use,
    Put,
    Talk,
    Go,
    Inventory,
    Help,
    Quit,
    Who,
    Login,
    Logout,
    Register,
    /// Unrecognized; keep the raw verb so Lua/room handlers can try.
    Unknown,
    /// Special commands starting with '@'
    ScBlueprint,
    ScPlaytest,
    // ScScript,
    ScDebug,
}

impl Verb {
    pub fn as_str(&self) -> &str {
        match self {
            Verb::Look => "look",
            Verb::Examine => "examine",
            Verb::Search => "search",
            Verb::Take => "take",
            Verb::Drop => "drop",
            Verb::Open => "open",
            Verb::Close => "close",
            Verb::Unlock => "unlock",
            Verb::Lock => "lock",
            Verb::Use => "use",
            Verb::Put => "put",
            Verb::Talk => "talk",
            Verb::Go => "go",
            Verb::Inventory => "inventory",
            Verb::Help => "help",
            Verb::Quit => "quit",
            Verb::Who => "who",
            Verb::Login => "login",
            Verb::Logout => "logout",
            Verb::Register => "register",
            Verb::Unknown => "unknown",
            Verb::ScBlueprint => "@bp",
            Verb::ScPlaytest => "@playtest",
            // Verb::ScScript => "@script",
            Verb::ScDebug => "@debug",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preposition {
    At,
    To,
    With,
    On,
    In,
    From,
    Through,
    Off,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quantifier {
    All,
}

#[derive(Debug, Clone)]
pub struct NounPhrase {
    /// Original (normalized) substring for this NP (articles removed).
    pub raw: String,
    /// Head noun (usually last token of the NP).
    pub head: String,
    /// Adjectives/modifiers (tokens before the head).
    pub adjectives: Vec<String>,
    /// Whether the NP came from a quoted token (e.g. "red access card").
    pub quoted: bool,
}

#[derive(Debug, Clone)]
pub struct Intent {
    pub verb: Verb,
    pub original: String,
    pub args: Vec<String>, // The raw args after the verb

    // Common slots
    pub direct: Option<NounPhrase>,
    pub target: Option<NounPhrase>,
    pub instrument: Option<NounPhrase>,
    pub preposition: Option<Preposition>,

    // Movement / special
    pub direction: Option<Direction>,

    // Quantity
    pub quantifier: Option<Quantifier>,

    /// Optional list of objects (e.g. "take coin, screwdriver and key")
    pub objects: Vec<NounPhrase>,

    /// If we couldn't canonicalize the verb, keep it here for scripting.
    pub raw_verb: Option<String>,
}

#[derive(Debug, Clone)]
struct Token {
    raw: String,   // as in input (lowercased by normalization)
    lower: String, // redundant, but explicit
    quoted: bool,
}

pub fn parse_command(input: &str) -> Intent {
    let normalized = normalize(input);
    let tokens = tokenize(&normalized);

    // Short-circuit: blank input
    if tokens.is_empty() {
        return Intent {
            verb: Verb::Unknown,
            args: vec![],
            original: normalized,
            direct: None,
            target: None,
            instrument: None,
            preposition: None,
            direction: None,
            quantifier: None,
            objects: vec![],
            raw_verb: None,
        };
    }

    // Directions-only shortcuts: "n", "north", etc.
    if let Some(dir) = Direction::parse(tokens[0].lower.as_str()) {
        return Intent {
            verb: Verb::Go,
            args: vec![],
            original: normalized,
            direct: None,
            target: None,
            instrument: None,
            preposition: None,
            direction: Some(dir),
            quantifier: None,
            objects: vec![],
            raw_verb: None,
        };
    }

    // Identify verb (phrasal first, then single)
    let (verb, consumed, forced_prep, raw_verb) = detect_verb(&tokens);

    // Movement form "go north"
    if verb == Verb::Go {
        let dir = tokens.get(consumed).and_then(|t| Direction::parse(t.lower.as_str()));

        return Intent {
            verb,
            args: tokens.iter().map(|t| t.lower.clone()).collect(),
            original: normalized,
            direct: None,
            target: None,
            instrument: None,
            preposition: None,
            direction: dir,
            quantifier: None,
            objects: vec![],
            raw_verb,
        };
    }

    // Remaining tokens after the verb
    let rest = &tokens[consumed..];

    // If a phrasal verb brought its own preposition ("look at", "talk to", "put in", ...)
    // we treat that as the canonical preposition, but still tolerate the user repeating it.
    let mut forced_prep = forced_prep;

    // If this verb often takes object lists (take/drop), try to split on ',' / 'and'
    let list_friendly = matches!(verb, Verb::Take | Verb::Drop | Verb::Put);
    let (mut pre_slot, post_slot, detected_prep) = split_on_preposition(rest, forced_prep);

    if detected_prep.is_some() && forced_prep.is_none() {
        forced_prep = detected_prep; // respect explicit preposition
    }

    // Quantifier (e.g., "all") typically sits before the direct object
    let quantifier = extract_quantifier(&mut pre_slot);

    // Build noun phrases
    let direct_objects = if list_friendly {
        parse_list_nps(&pre_slot)
    } else {
        maybe_np(&pre_slot).into_iter().collect()
    };

    let (instrument, target) = match forced_prep {
        Some(Preposition::With) => (maybe_np(&post_slot), None),
        Some(_) => (None, maybe_np(&post_slot)),
        None => (None, None),
    };

    // Choose a primary direct NP if present (first list item)
    let direct = direct_objects.get(0).cloned();

    Intent {
        verb,
        args: tokens.iter().map(|t| t.lower.clone()).collect(),
        original: normalized,
        direct,
        target,
        instrument,
        preposition: forced_prep,
        direction: None,
        quantifier,
        // If there were multiple objects for verbs like "take", keep them here
        objects: direct_objects,
        raw_verb,
    }
}

//
// ---- Normalization & tokenization ----
//

fn normalize(s: &str) -> String {
    // lowercase, trim, collapse spaces
    let mut out = String::with_capacity(s.len());
    let mut last_space = false;
    for ch in s.trim().chars() {
        let c = ch.to_ascii_lowercase();
        if c.is_whitespace() {
            if !last_space {
                out.push(' ');
                last_space = true;
            }
        } else {
            out.push(c);
            last_space = false;
        }
    }
    out
}

fn tokenize(s: &str) -> Vec<Token> {
    let mut toks = Vec::new();
    let mut buf = String::new();
    let mut in_quote: Option<char> = None;

    let push_tok = |quoted: bool, buf: &mut String, toks: &mut Vec<Token>| {
        if !buf.is_empty() {
            let raw = buf.clone();
            toks.push(Token {
                lower: raw.clone(),
                raw,
                quoted,
            });
            buf.clear();
        }
    };

    for ch in s.chars() {
        match in_quote {
            Some(q) if ch == q => {
                // end quote
                push_tok(true, &mut buf, &mut toks);
                in_quote = None;
            }
            Some(_) => {
                buf.push(ch);
            }
            None => {
                match ch {
                    '"' | '\'' => {
                        if !buf.is_empty() {
                            // starting a quote right after text → split
                            push_tok(false, &mut buf, &mut toks);
                        }
                        in_quote = Some(ch);
                    }
                    ' ' => {
                        push_tok(false, &mut buf, &mut toks);
                    }
                    _ => buf.push(ch),
                }
            }
        }
    }
    push_tok(in_quote.is_some(), &mut buf, &mut toks);
    toks
}

//
// ---- Verb detection ----
//

fn detect_verb(tokens: &[Token]) -> (Verb, usize, Option<Preposition>, Option<String>) {
    // let mut raw_verb: Option<String> = None;

    // Phrasal verbs (2-word) that imply a preposition or canonical verb
    if tokens.len() >= 2 {
        let a = tokens[0].lower.as_str();
        let b = tokens[1].lower.as_str();
        match (a, b) {
            ("pick", "up") => return (Verb::Take, 2, None, None),
            ("look", "at") => return (Verb::Look, 2, Some(Preposition::At), None),
            ("turn", "on") => return (Verb::Use, 2, Some(Preposition::On), None),
            ("turn", "off") => return (Verb::Use, 2, Some(Preposition::Off), None),
            ("put", "in") | ("put", "into") => return (Verb::Put, 2, Some(Preposition::In), None),
            ("put", "on") | ("put", "onto") => return (Verb::Put, 2, Some(Preposition::On), None),
            ("talk", "to") => return (Verb::Talk, 2, Some(Preposition::To), None),
            ("give", "to") => return (Verb::Use, 2, Some(Preposition::To), None), // map to Use/Give later if needed
            _ => {}
        }
    }

    // Single-word verbs (with synonyms)
    let verb_map = verb_map();
    let a = tokens[0].lower.as_str();
    if let Some(v) = verb_map.get(a).copied() {
        return (v, 1, None, None);
    }

    // "go <direction>" typed as "north" is handled earlier; here "go" already mapped if needed.
    // Unknown verb: pass the raw string upward
    let raw_verb = Some(tokens[0].raw.clone());
    (Verb::Unknown, 1, None, raw_verb)
}

fn verb_map() -> HashMap<&'static str, Verb> {
    use Verb::*;
    let mut m = HashMap::new();
    // look
    for k in ["look", "l"].iter() {
        m.insert(*k, Look);
    }
    // examine
    for k in ["examine", "x", "inspect"].iter() {
        m.insert(*k, Examine);
    }
    m.insert("search", Search);
    // take
    for k in ["take", "get", "grab"].iter() {
        m.insert(*k, Take);
    }
    // drop
    m.insert("drop", Drop);
    // open/close/lock/unlock
    m.insert("open", Open);
    m.insert("close", Close);
    m.insert("unlock", Unlock);
    m.insert("lock", Lock);
    // use
    m.insert("use", Use);
    // put
    m.insert("put", Put);
    // talk
    for k in ["talk", "speak", "say"].iter() {
        m.insert(*k, Talk);
    }
    // go / move
    for k in ["go", "walk", "move"].iter() {
        m.insert(*k, Go);
    }
    // inventory
    for k in ["inventory", "inv", "i"].iter() {
        m.insert(*k, Inventory);
    }
    // who
    for k in ["whoami", "who"].iter() {
        m.insert(*k, Who);
    }

    // help, quit
    m.insert("help", Help);
    m.insert("?", Help);
    for k in ["quit", "exit"].iter() {
        m.insert(*k, Quit);
    }

    m.insert("login", Login);
    m.insert("logout", Logout);
    m.insert("register", Register);

    // Special commands starting with '@'
    m.insert("@bp", ScBlueprint);
    m.insert("@playtest", ScPlaytest);
    m.insert("@debug", ScDebug);

    m
}

//
// ---- Prepositions / directions / quantifier ----
//

fn canonical_prep(s: &str) -> Option<Preposition> {
    match s {
        "at" => Some(Preposition::At),
        "to" => Some(Preposition::To),
        "with" | "using" => Some(Preposition::With),
        "on" | "onto" => Some(Preposition::On),
        "in" | "into" => Some(Preposition::In),
        "from" => Some(Preposition::From),
        "through" => Some(Preposition::Through),
        "off" => Some(Preposition::Off),
        _ => None,
    }
}

fn extract_quantifier(pre_tokens: &mut Vec<Token>) -> Option<Quantifier> {
    if pre_tokens.first().map(|t| t.lower.as_str()) == Some("all")
        || pre_tokens.first().map(|t| t.lower.as_str()) == Some("everything")
    {
        pre_tokens.remove(0);
        Some(Quantifier::All)
    } else {
        None
    }
}

//
// ---- Splitting around prepositions ----
//

/// Split the remaining tokens into [pre] PREP [post].
/// If `forced_prep` is provided (from a phrasal verb), we use it if present,
/// otherwise we scan for the first known preposition.
fn split_on_preposition(
    tokens: &[Token],
    forced_prep: Option<Preposition>,
) -> (Vec<Token>, Vec<Token>, Option<Preposition>) {
    if tokens.is_empty() {
        return (vec![], vec![], forced_prep);
    }

    // If the first token literally equals the forced preposition, drop it.
    if let Some(fp) = forced_prep {
        if let Some(first) = tokens.first() {
            if canonical_prep(&first.lower) == Some(fp) {
                return (vec![], tokens[1..].to_vec(), Some(fp));
            }
        }
        // Otherwise keep scanning as normal but prefer the forced preposition if encountered.
    }

    for (i, tok) in tokens.iter().enumerate() {
        if let Some(p) = canonical_prep(&tok.lower) {
            let pre = tokens[..i].to_vec();
            let post = tokens[i + 1..].to_vec();
            return (pre, post, Some(p));
        }
    }

    (tokens.to_vec(), vec![], forced_prep)
}

//
// ---- Noun phrase helpers ----
//

fn maybe_np(tokens: &[Token]) -> Option<NounPhrase> {
    let cleaned = strip_determiners(tokens);
    if cleaned.is_empty() {
        return None;
    }
    Some(build_np(&cleaned))
}

fn parse_list_nps(tokens: &[Token]) -> Vec<NounPhrase> {
    // Split on ','-style punctuation and the word "and".
    // Our tokenizer doesn't break punctuation off tokens, so detect trailing commas.
    let mut groups: Vec<Vec<Token>> = vec![Vec::new()];

    for mut t in tokens.iter().cloned() {
        let is_and = t.lower == "and";
        let is_lonely_comma = t.lower == ","; // Just in case your input injects a lone comma.
        let had_trailing_comma = t.lower.ends_with(',');

        // If token ends with ',', trim it off for NP building.
        if had_trailing_comma && t.lower.len() >= 1 {
            t.lower.pop();
            t.raw.pop();
        }

        if is_and || is_lonely_comma {
            // Boundary between list items
            if !groups.last().unwrap().is_empty() {
                groups.push(Vec::new());
            }
        } else {
            groups.last_mut().unwrap().push(t);
            if had_trailing_comma {
                groups.push(Vec::new());
            }
        }
    }

    groups
        .into_iter()
        .filter_map(|g| {
            let g2 = strip_determiners(&g);
            if g2.is_empty() { None } else { Some(build_np(&g2)) }
        })
        .collect()
}

fn strip_determiners(tokens: &[Token]) -> Vec<Token> {
    if tokens.is_empty() {
        return vec![];
    }
    // Common determiners/articles/pronouns that shouldn't be part of the NP
    let dets: HashSet<&'static str> = [
        "a", "an", "the", "some", "my", "your", "his", "her", "their", "our", "this", "that", "these", "those",
    ]
    .into_iter()
    .collect();

    tokens
        .iter()
        .filter(|t| !dets.contains(t.lower.as_str()))
        .cloned()
        .collect()
}

fn build_np(tokens: &[Token]) -> NounPhrase {
    // Join raw with spaces (already normalized)
    let raw = tokens.iter().map(|t| t.raw.as_str()).collect::<Vec<_>>().join(" ");

    // If it's a single quoted token, we can derive head as last word inside
    let quoted = tokens.len() == 1 && tokens[0].quoted;

    let words: Vec<String>;
    if quoted {
        // Split the quoted multiword into words for head/adjectives
        words = tokens[0].raw.split_whitespace().map(|s| s.to_string()).collect();
    } else {
        words = tokens.iter().map(|t| t.raw.clone()).collect();
    }

    let head = words.last().cloned().unwrap_or_else(|| raw.clone());
    let adjectives = if words.len() > 1 {
        words[..words.len() - 1].to_vec()
    } else {
        vec![]
    };

    NounPhrase {
        raw,
        head,
        adjectives,
        quoted,
    }
}

//
// ---- Tests (basic) ----
//

#[cfg(test)]
mod tests {
    use super::*;

    fn d(np: &NounPhrase) -> (&str, &str, Vec<&str>) {
        (
            np.raw.as_str(),
            np.head.as_str(),
            np.adjectives.iter().map(|s| s.as_str()).collect(),
        )
    }

    #[test]
    fn t_open_door() {
        let i = parse_command("open the door");
        assert_eq!(i.verb, Verb::Open);
        assert!(i.direct.is_some());
        let (raw, head, _adjs) = d(i.direct.as_ref().unwrap());
        assert_eq!(raw, "door");
        assert_eq!(head, "door");
        assert!(i.instrument.is_none());

        assert_eq!(i.args[0], "open");
        assert_eq!(i.args[1], "the");
        assert_eq!(i.args[2], "door");
    }

    #[test]
    fn t_open_with_key() {
        let i = parse_command("open door with key");
        assert_eq!(i.verb, Verb::Open);
        assert_eq!(i.preposition, Some(Preposition::With));
        assert_eq!(i.instrument.as_ref().unwrap().head, "key");
    }

    #[test]
    fn t_look_at() {
        let i = parse_command("look at markings");
        assert_eq!(i.verb, Verb::Look);
        assert_eq!(i.preposition, Some(Preposition::At));
        assert_eq!(i.direct.unwrap().head, "markings");
    }

    #[test]
    fn t_put_into() {
        let i = parse_command("put coin into toolkit");
        assert_eq!(i.verb, Verb::Put);
        assert_eq!(i.preposition, Some(Preposition::In));
        assert_eq!(i.direct.unwrap().head, "coin");
        assert_eq!(i.target.unwrap().head, "toolkit");
    }

    #[test]
    fn t_take_all_from() {
        let i = parse_command("take all coins from bag");
        assert_eq!(i.verb, Verb::Take);
        assert_eq!(i.quantifier, Some(Quantifier::All));
        assert_eq!(i.preposition, Some(Preposition::From));
        assert_eq!(i.direct.unwrap().head, "coins");
        assert_eq!(i.target.unwrap().head, "bag");
    }

    #[test]
    fn t_direction_shortcut() {
        let i = parse_command("n");
        assert_eq!(i.verb, Verb::Go);
        assert_eq!(i.direction, Some(Direction::North));
    }

    #[test]
    fn t_picked_up_synonym() {
        let i = parse_command("pick up screwdriver");
        assert_eq!(i.verb, Verb::Take);
        assert_eq!(i.direct.unwrap().head, "screwdriver");
    }

    #[test]
    fn t_list_take() {
        let i = parse_command("take coin, screwdriver and key");
        assert_eq!(i.verb, Verb::Take);
        assert_eq!(i.objects.len(), 3);
        assert_eq!(i.objects[0].head, "coin");
        assert_eq!(i.objects[1].head, "screwdriver");
        assert_eq!(i.objects[2].head, "key");

        assert_eq!(i.args[0], "take");
        assert_eq!(i.args[1], "coin,");
        assert_eq!(i.args[2], "screwdriver");
        assert_eq!(i.args[3], "and");
        assert_eq!(i.args[4], "key");
    }

    #[test]
    fn t_open_articles_and_spaces() {
        let i = parse_command("   Open   The   Door   ");
        assert_eq!(i.verb, Verb::Open);
        assert_eq!(i.direct.unwrap().head, "door");
    }

    #[test]
    fn t_open_using_synonym() {
        let i = parse_command("open door using key");
        assert_eq!(i.verb, Verb::Open);
        assert_eq!(i.preposition, Some(Preposition::With));
        assert_eq!(i.instrument.unwrap().head, "key");
    }

    #[test]
    fn t_put_on_target() {
        let i = parse_command("put coin on altar");
        assert_eq!(i.verb, Verb::Put);
        assert_eq!(i.preposition, Some(Preposition::On));
        assert_eq!(i.direct.unwrap().head, "coin");
        assert_eq!(i.target.unwrap().head, "altar");
    }

    #[test]
    fn t_go_full_word() {
        let i = parse_command("go west");
        assert_eq!(i.verb, Verb::Go);
        assert_eq!(i.direction, Some(Direction::West));
    }

    #[test]
    fn t_unknown_verb_kept_raw() {
        let i = parse_command("frobnicate lever");
        assert_eq!(i.verb, Verb::Unknown);
        assert_eq!(i.raw_verb.as_deref(), Some("frobnicate"));
        assert_eq!(i.direct.unwrap().head, "lever");
    }

    #[test]
    fn t_quoted_multiword_noun() {
        let i = parse_command(r#"take "red access card""#);
        assert_eq!(i.verb, Verb::Take);
        let np = i.direct.unwrap();
        assert_eq!(np.raw, "red access card");
        assert_eq!(np.head, "card");
        assert_eq!(np.adjectives, vec!["red", "access"]);
        assert!(np.quoted);

        assert_eq!(i.args[0], "take");
        assert_eq!(i.args[1], "red access card");
    }

    #[test]
    fn t_unquoted_multiword_noun() {
        let i = parse_command("look at red access panel");
        assert_eq!(i.verb, Verb::Look);
        let np = i.direct.unwrap();
        assert_eq!(np.head, "panel");
        assert_eq!(np.adjectives, vec!["red", "access"]);
    }

    #[test]
    fn t_talk_to_npc() {
        let i = parse_command("talk to technician");
        assert_eq!(i.verb, Verb::Talk);
        assert_eq!(i.preposition, Some(Preposition::To));
        // With our current model, the noun sits in `direct`
        assert_eq!(i.direct.unwrap().head, "technician");
    }

    #[test]
    fn t_turn_off_console() {
        let i = parse_command("turn off console");
        assert_eq!(i.verb, Verb::Use);
        assert_eq!(i.preposition, Some(Preposition::Off));
        assert_eq!(i.direct.unwrap().head, "console");
    }

    #[test]
    fn t_take_from_container() {
        let i = parse_command("take coin from bag");
        assert_eq!(i.verb, Verb::Take);
        assert_eq!(i.preposition, Some(Preposition::From));
        assert_eq!(i.direct.unwrap().head, "coin");
        assert_eq!(i.target.unwrap().head, "bag");
    }

    #[test]
    fn t_list_with_commas_and_and() {
        let i = parse_command("take coin, screwdriver, and key");
        assert_eq!(i.verb, Verb::Take);
        assert_eq!(i.objects.len(), 3);
        assert_eq!(i.objects[0].head, "coin");
        assert_eq!(i.objects[1].head, "screwdriver");
        assert_eq!(i.objects[2].head, "key");
    }

    #[test]
    fn t_list_with_commas_no_and() {
        let i = parse_command("take coin, screwdriver, key");
        assert_eq!(i.objects.len(), 3);
        assert_eq!(i.objects[0].head, "coin");
        assert_eq!(i.objects[1].head, "screwdriver");
        assert_eq!(i.objects[2].head, "key");
    }

    #[test]
    fn t_everything_quantifier() {
        let i = parse_command("take everything");
        assert_eq!(i.verb, Verb::Take);
        assert_eq!(i.quantifier, Some(Quantifier::All));
        assert!(i.direct.is_none());
        assert!(i.objects.is_empty());
    }

    #[test]
    fn t_inventory_alias() {
        let i = parse_command("i");
        assert_eq!(i.verb, Verb::Inventory);
    }

    #[test]
    fn t_quit_alias() {
        let i = parse_command("exit");
        assert_eq!(i.verb, Verb::Quit);
    }

    #[test]
    fn t_direction_shortcut_uppercase() {
        let i = parse_command("N");
        assert_eq!(i.verb, Verb::Go);
        assert_eq!(i.direction, Some(Direction::North));
    }

    #[test]
    fn t_put_into_alias() {
        let i = parse_command("put coins into bag");
        assert_eq!(i.verb, Verb::Put);
        assert_eq!(i.preposition, Some(Preposition::In));
        assert_eq!(i.direct.unwrap().head, "coins");
        assert_eq!(i.target.unwrap().head, "bag");
    }

    #[test]
    fn t_strip_possessive_determiners() {
        let i = parse_command("take my key");
        assert_eq!(i.verb, Verb::Take);
        assert_eq!(i.direct.unwrap().raw, "key");
    }

    #[test]
    fn t_and_with_quoted_items() {
        let i = parse_command(r#"take "red card" and "blue key""#);
        assert_eq!(i.verb, Verb::Take);
        assert_eq!(i.objects.len(), 2);
        assert_eq!(i.objects[0].raw, "red card");
        assert_eq!(i.objects[1].raw, "blue key");

        assert_eq!(i.args[0], "take");
        assert_eq!(i.args[1], "red card");
        assert_eq!(i.args[2], "and");
        assert_eq!(i.args[3], "blue key");
    }

    #[test]
    fn t_look_bare() {
        let i = parse_command("look");
        assert_eq!(i.verb, Verb::Look);
        assert!(i.direct.is_none());
    }

    #[test]
    fn t_open_redundant_prep() {
        // Even if user repeats the preposition after a phrasal verb,
        // our split logic should still pick a direct NP cleanly.
        let i = parse_command("look at the markings");
        assert_eq!(i.verb, Verb::Look);
        assert_eq!(i.preposition, Some(Preposition::At));
        assert_eq!(i.direct.unwrap().head, "markings");
    }
}
