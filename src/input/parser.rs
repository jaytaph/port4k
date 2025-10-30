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

#[derive(Debug, Clone, PartialEq, Eq)]
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
    LuaRepl,
    Register,
    /// Special commands starting with '@'
    ScBlueprint,
    ScPlaytest,
    ScDebug,
    /// Custom verb not in our known list
    Custom(String),
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
            Verb::LuaRepl => "lua",
            Verb::ScBlueprint => "@bp",
            Verb::ScPlaytest => "@playtest",
            Verb::ScDebug => "@debug",
            Verb::Custom(s) => s.as_str(),
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

impl Preposition {
    pub fn as_str(&self) -> &str {
        match self {
            Preposition::At => "at",
            Preposition::To => "to",
            Preposition::With => "with",
            Preposition::On => "on",
            Preposition::In => "in",
            Preposition::From => "from",
            Preposition::Through => "through",
            Preposition::Off => "off",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quantifier {
    All,
}

impl Quantifier {
    pub fn as_str(&self) -> &str {
        match self {
            Quantifier::All => "all",
        }
    }
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

impl std::fmt::Display for NounPhrase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.raw)
    }
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

    // Raw direct data (for number, codes etc)
    pub direct_raw: Option<String>,

    // Movement / special
    pub direction: Option<Direction>,

    // Quantity
    pub quantifier: Option<Quantifier>,

    /// Optional list of objects (e.g. "take coin, screwdriver and key")
    pub objects: Vec<NounPhrase>,
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
            verb: Verb::Custom("".to_string()),
            args: vec![],
            original: normalized,
            direct: None,
            direct_raw: None,
            target: None,
            instrument: None,
            preposition: None,
            direction: None,
            quantifier: None,
            objects: vec![],
        };
    }

    // Directions-only shortcuts: "n", "north", etc.
    if let Some(dir) = Direction::parse(tokens[0].lower.as_str()) {
        return Intent {
            verb: Verb::Go,
            args: vec![],
            original: normalized,
            direct: None,
            direct_raw: None,
            target: None,
            instrument: None,
            preposition: None,
            direction: Some(dir),
            quantifier: None,
            objects: vec![],
        };
    }

    // Identify verb (phrasal first, then single)
    let (verb, consumed, forced_prep, _raw_verb) = detect_verb(&tokens);

    // Movement form "go north"
    if verb == Verb::Go {
        let dir = tokens.get(consumed).and_then(|t| Direction::parse(t.lower.as_str()));

        return Intent {
            verb,
            args: tokens.iter().map(|t| t.lower.clone()).collect(),
            original: normalized,
            direct: None,
            direct_raw: None,
            target: None,
            instrument: None,
            preposition: None,
            direction: dir,
            quantifier: None,
            objects: vec![],
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

    let direct_raw = if !pre_slot.is_empty() && is_raw_data(&pre_slot) {
        Some(pre_slot.iter().map(|t| t.raw.as_str()).collect::<Vec<_>>().join(" "))
    } else {
        None
    };

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
    let direct = direct_objects.first().cloned();

    Intent {
        verb,
        args: tokens.iter().map(|t| t.lower.clone()).collect(),
        original: normalized,
        direct,
        direct_raw,
        target,
        instrument,
        preposition: forced_prep,
        direction: None,
        quantifier,
        // If there were multiple objects for verbs like "take", keep them here
        objects: direct_objects,
    }
}

// Helper to detect if tokens look like raw data (numbers, codes)
fn is_raw_data(tokens: &[Token]) -> bool {
    if tokens.is_empty() {
        return false;
    }

    // Check if it's all digits/numbers
    let all_numeric = tokens
        .iter()
        .all(|t| t.raw.chars().all(|c| c.is_ascii_digit() || c == '-' || c == '.'));

    all_numeric
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
            ("give", "to") => return (Verb::Use, 2, Some(Preposition::To), None),
            _ => {}
        }
    }

    // Single-word verbs (with synonyms)
    let verb_map = verb_map();
    let a = tokens[0].lower.as_str();
    if let Some(v) = verb_map.get(a) {
        return (v.clone(), 1, None, None);
    }

    // Custom/unknown verb: pass as Custom variant
    let raw_verb = tokens[0].raw.clone();
    (Verb::Custom(raw_verb.clone()), 1, None, Some(raw_verb))
}

fn verb_map() -> HashMap<&'static str, Verb> {
    use Verb::*;
    let mut m = HashMap::new();
    // lua repl
    for k in ["lua", "luarepl", "repl"].iter() {
        m.insert(*k, LuaRepl);
    }
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
    if let Some(fp) = forced_prep
        && let Some(first) = tokens.first()
        && canonical_prep(&first.lower) == Some(fp)
    {
        return (vec![], tokens[1..].to_vec(), Some(fp));
    }
    // Otherwise keep scanning as normal but prefer the forced preposition if encountered.

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
        if had_trailing_comma && !t.lower.is_empty() {
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

    let words: Vec<String> = if quoted {
        // Split the quoted multiword into words for head/adjectives
        tokens[0].raw.split_whitespace().map(|s| s.to_string()).collect()
    } else {
        tokens.iter().map(|t| t.raw.clone()).collect()
    };

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
        assert_eq!(i.verb, Verb::Custom("frobnicate".to_string()));
        assert_eq!(i.direct_raw.as_deref(), None);
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

    #[test]
    fn t_enter_code_on_panel() {
        let i = parse_command("enter 1234 on panel");
        assert_eq!(i.verb, Verb::Custom("enter".to_string()));
        assert_eq!(i.direct_raw.as_deref(), Some("1234"));
        assert_eq!(i.preposition, Some(Preposition::On));
        assert_eq!(i.target.unwrap().head, "panel");
    }

    #[test]
    fn t_shatter_with_hammer() {
        let i = parse_command("shatter glass door with hammer");
        assert_eq!(i.verb, Verb::Custom("shatter".to_string()));
        assert_eq!(i.direct.as_ref().unwrap().raw, "glass door");
        assert_eq!(i.preposition, Some(Preposition::With));
        assert_eq!(i.instrument.unwrap().head, "hammer");
    }

    #[test]
    fn t_custom_verb_simple() {
        let i = parse_command("dance");
        assert_eq!(i.verb, Verb::Custom("dance".to_string()));
        assert!(i.direct.is_none());
        assert!(i.direct_raw.is_none());
    }

    #[test]
    fn t_custom_verb_with_object() {
        let i = parse_command("kick ball");
        assert_eq!(i.verb, Verb::Custom("kick".to_string()));
        assert_eq!(i.direct.unwrap().head, "ball");
    }

    #[test]
    fn t_custom_verb_with_prep() {
        let i = parse_command("climb on ladder");
        assert_eq!(i.verb, Verb::Custom("climb".to_string()));
        assert_eq!(i.preposition, Some(Preposition::On));
        assert_eq!(i.target.unwrap().head, "ladder");
    }

    #[test]
    fn t_custom_verb_multiword_object() {
        let i = parse_command("break wooden crate with crowbar");
        assert_eq!(i.verb, Verb::Custom("break".to_string()));
        assert_eq!(i.direct.as_ref().unwrap().raw, "wooden crate");
        assert_eq!(i.direct.as_ref().unwrap().head, "crate");
        assert_eq!(i.direct.as_ref().unwrap().adjectives, vec!["wooden"]);
        assert_eq!(i.preposition, Some(Preposition::With));
        assert_eq!(i.instrument.unwrap().head, "crowbar");
    }

    // ---- Raw data tests ----

    #[test]
    fn t_enter_simple_code() {
        let i = parse_command("enter 4312");
        dbg!(&i);
        assert_eq!(i.verb, Verb::Custom("enter".to_string()));
        assert_eq!(i.direct_raw.as_deref(), Some("4312"));
        assert!(i.direct.unwrap().head == "4312");
    }

    #[test]
    fn t_type_code_with_dashes() {
        let i = parse_command("type 123-456");
        assert_eq!(i.verb, Verb::Custom("type".to_string()));
        assert_eq!(i.direct_raw.as_deref(), Some("123-456"));
    }

    #[test]
    fn t_input_decimal() {
        let i = parse_command("input 3.14 into console");
        assert_eq!(i.verb, Verb::Custom("input".to_string()));
        assert_eq!(i.direct_raw.as_deref(), Some("3.14"));
        assert_eq!(i.preposition, Some(Preposition::In));
        assert_eq!(i.target.unwrap().head, "console");
    }

    #[test]
    fn t_dial_number() {
        let i = parse_command("dial 555-1234 on phone");
        assert_eq!(i.verb, Verb::Custom("dial".to_string()));
        assert_eq!(i.direct_raw.as_deref(), Some("555-1234"));
        assert_eq!(i.preposition, Some(Preposition::On));
        assert_eq!(i.target.unwrap().head, "phone");
    }

    // ---- Preposition edge cases ----

    #[test]
    fn t_multiple_preps_first_wins() {
        let i = parse_command("put coin in box on table");
        assert_eq!(i.verb, Verb::Put);
        assert_eq!(i.direct.unwrap().head, "coin");
        assert_eq!(i.preposition, Some(Preposition::In));
        // "box on table" becomes the target
        assert_eq!(i.target.as_ref().unwrap().raw, "box on table");
    }

    #[test]
    fn t_preposition_at_start_no_object() {
        let i = parse_command("look at");
        assert_eq!(i.verb, Verb::Look);
        assert_eq!(i.preposition, Some(Preposition::At));
        assert!(i.direct.is_none());
    }

    #[test]
    fn t_using_synonym_for_with() {
        let i = parse_command("cut rope using knife");
        assert_eq!(i.verb, Verb::Custom("cut".to_string()));
        assert_eq!(i.direct.unwrap().head, "rope");
        assert_eq!(i.preposition, Some(Preposition::With));
        assert_eq!(i.instrument.unwrap().head, "knife");
    }

    #[test]
    fn t_into_vs_in() {
        let i = parse_command("pour water into glass");
        assert_eq!(i.verb, Verb::Custom("pour".to_string()));
        assert_eq!(i.direct.unwrap().head, "water");
        assert_eq!(i.preposition, Some(Preposition::In));
        assert_eq!(i.target.unwrap().head, "glass");
    }

    #[test]
    fn t_onto_vs_on() {
        let i = parse_command("jump onto platform");
        assert_eq!(i.verb, Verb::Custom("jump".to_string()));
        assert_eq!(i.preposition, Some(Preposition::On));
        assert_eq!(i.target.unwrap().head, "platform");
    }

    // ---- Quoted strings ----

    #[test]
    fn t_single_quoted_string() {
        let i = parse_command("read 'warning sign'");
        assert_eq!(i.verb, Verb::Custom("read".to_string()));
        let np = i.direct.unwrap();
        assert_eq!(np.raw, "warning sign");
        assert_eq!(np.head, "sign");
        assert!(np.quoted);
    }

    #[test]
    fn t_quoted_with_preposition() {
        let i = parse_command(r#"put "rusty key" in "metal box""#);
        assert_eq!(i.verb, Verb::Put);
        assert_eq!(i.direct.as_ref().unwrap().raw, "rusty key");
        assert!(i.direct.as_ref().unwrap().quoted);
        assert_eq!(i.preposition, Some(Preposition::In));
        assert_eq!(i.target.as_ref().unwrap().raw, "metal box");
        assert!(i.target.as_ref().unwrap().quoted);
    }

    #[test]
    fn t_mixed_quoted_unquoted() {
        let i = parse_command(r#"use "red keycard" on scanner"#);
        assert_eq!(i.verb, Verb::Use);
        assert_eq!(i.direct.as_ref().unwrap().raw, "red keycard");
        assert!(i.direct.as_ref().unwrap().quoted);
        assert_eq!(i.preposition, Some(Preposition::On));
        assert_eq!(i.target.as_ref().unwrap().head, "scanner");
        assert!(!i.target.as_ref().unwrap().quoted);
    }

    // ---- Quantifiers ----

    #[test]
    fn t_all_with_preposition() {
        let i = parse_command("take all items from chest");
        assert_eq!(i.verb, Verb::Take);
        assert_eq!(i.quantifier, Some(Quantifier::All));
        assert_eq!(i.direct.as_ref().unwrap().head, "items");
        assert_eq!(i.preposition, Some(Preposition::From));
        assert_eq!(i.target.unwrap().head, "chest");
    }

    #[test]
    fn t_everything_no_object() {
        let i = parse_command("drop everything");
        assert_eq!(i.verb, Verb::Drop);
        assert_eq!(i.quantifier, Some(Quantifier::All));
        assert!(i.direct.is_none());
    }

    // ---- Complex noun phrases ----

    #[test]
    fn t_three_word_noun() {
        let i = parse_command("examine old rusty lock");
        assert_eq!(i.verb, Verb::Examine);
        let np = i.direct.unwrap();
        assert_eq!(np.raw, "old rusty lock");
        assert_eq!(np.head, "lock");
        assert_eq!(np.adjectives, vec!["old", "rusty"]);
    }

    #[test]
    fn t_determiners_stripped() {
        let i = parse_command("take the big red ball");
        assert_eq!(i.verb, Verb::Take);
        let np = i.direct.unwrap();
        assert_eq!(np.raw, "big red ball");
        assert_eq!(np.head, "ball");
        assert_eq!(np.adjectives, vec!["big", "red"]);
    }

    #[test]
    fn t_possessive_stripped() {
        let i = parse_command("open my old wooden chest");
        assert_eq!(i.verb, Verb::Open);
        let np = i.direct.unwrap();
        assert_eq!(np.raw, "old wooden chest");
        assert_eq!(np.head, "chest");
    }

    // ---- List parsing ----

    #[test]
    fn t_list_no_and() {
        let i = parse_command("drop coin, key, rope");
        assert_eq!(i.verb, Verb::Drop);
        assert_eq!(i.objects.len(), 3);
        assert_eq!(i.objects[0].head, "coin");
        assert_eq!(i.objects[1].head, "key");
        assert_eq!(i.objects[2].head, "rope");
    }

    #[test]
    fn t_list_multiword_items() {
        let i = parse_command("take red key, blue card and green gem");
        assert_eq!(i.verb, Verb::Take);
        assert_eq!(i.objects.len(), 3);
        assert_eq!(i.objects[0].raw, "red key");
        assert_eq!(i.objects[1].raw, "blue card");
        assert_eq!(i.objects[2].raw, "green gem");
    }

    #[test]
    fn t_list_with_oxford_comma() {
        let i = parse_command("get hammer, nails, and screwdriver");
        assert_eq!(i.verb, Verb::Take);
        assert_eq!(i.objects.len(), 3);
    }

    // ---- Direction tests ----

    #[test]
    fn t_all_cardinal_directions() {
        let dirs = vec![
            ("n", Direction::North),
            ("s", Direction::South),
            ("e", Direction::East),
            ("w", Direction::West),
            ("ne", Direction::Northeast),
            ("nw", Direction::Northwest),
            ("se", Direction::Southeast),
            ("sw", Direction::Southwest),
        ];

        for (cmd, expected_dir) in dirs {
            let i = parse_command(cmd);
            assert_eq!(i.verb, Verb::Go);
            assert_eq!(i.direction, Some(expected_dir));
        }
    }

    #[test]
    fn t_up_down() {
        let i = parse_command("up");
        assert_eq!(i.verb, Verb::Go);
        assert_eq!(i.direction, Some(Direction::Up));

        let i = parse_command("down");
        assert_eq!(i.verb, Verb::Go);
        assert_eq!(i.direction, Some(Direction::Down));
    }

    #[test]
    fn t_go_with_direction() {
        let i = parse_command("go northeast");
        assert_eq!(i.verb, Verb::Go);
        assert_eq!(i.direction, Some(Direction::Northeast));
    }

    // ---- Edge cases ----

    #[test]
    fn t_empty_string() {
        let i = parse_command("");
        assert!(matches!(i.verb, Verb::Custom(_)));
        assert!(i.args.is_empty());
    }

    #[test]
    fn t_whitespace_only() {
        let i = parse_command("   \t  \n  ");
        assert!(matches!(i.verb, Verb::Custom(_)));
        assert!(i.args.is_empty());
    }

    #[test]
    fn t_single_word_unknown() {
        let i = parse_command("xyzzy");
        assert_eq!(i.verb, Verb::Custom("xyzzy".to_string()));
        assert!(i.direct.is_none());
    }

    #[test]
    fn t_case_insensitive() {
        let i = parse_command("OPEN THE DOOR");
        assert_eq!(i.verb, Verb::Open);
        assert_eq!(i.direct.unwrap().head, "door");
    }

    #[test]
    fn t_mixed_case() {
        let i = parse_command("TaKe ReD kEy");
        assert_eq!(i.verb, Verb::Take);
        assert_eq!(i.direct.as_ref().unwrap().raw, "red key");
    }

    #[test]
    fn t_extra_spaces() {
        let i = parse_command("  look   at    the     panel  ");
        assert_eq!(i.verb, Verb::Look);
        assert_eq!(i.preposition, Some(Preposition::At));
        assert_eq!(i.direct.unwrap().head, "panel");
    }

    // ---- Phrasal verb tests ----

    #[test]
    fn t_pick_up_multiword() {
        let i = parse_command("pick up rusty screwdriver");
        assert_eq!(i.verb, Verb::Take);
        let np = i.direct.unwrap();
        assert_eq!(np.raw, "rusty screwdriver");
        assert_eq!(np.head, "screwdriver");
    }

    #[test]
    fn t_turn_on_off() {
        let i = parse_command("turn on generator");
        assert_eq!(i.verb, Verb::Use);
        assert_eq!(i.preposition, Some(Preposition::On));
        assert_eq!(i.direct.unwrap().head, "generator");

        let i = parse_command("turn off lights");
        assert_eq!(i.verb, Verb::Use);
        assert_eq!(i.preposition, Some(Preposition::Off));
        assert_eq!(i.direct.unwrap().head, "lights");
    }

    // ---- Special commands ----

    #[test]
    fn t_help_with_question_mark() {
        let i = parse_command("?");
        assert_eq!(i.verb, Verb::Help);
    }

    #[test]
    fn t_inventory_shortcuts() {
        for cmd in ["i", "inv", "inventory"] {
            let i = parse_command(cmd);
            assert_eq!(i.verb, Verb::Inventory);
        }
    }

    #[test]
    fn t_examine_shortcuts() {
        for cmd in ["x door", "examine door", "inspect door"] {
            let i = parse_command(cmd);
            assert_eq!(i.verb, Verb::Examine);
            assert_eq!(i.direct.as_ref().unwrap().head, "door");
        }
    }

    #[test]
    fn t_special_commands() {
        let i = parse_command("@bp");
        assert_eq!(i.verb, Verb::ScBlueprint);

        let i = parse_command("@playtest");
        assert_eq!(i.verb, Verb::ScPlaytest);

        let i = parse_command("@debug");
        assert_eq!(i.verb, Verb::ScDebug);
    }

    // ---- Args field tests ----

    #[test]
    fn t_args_preserved() {
        let i = parse_command("open the red door");
        assert_eq!(i.args, vec!["open", "the", "red", "door"]);
    }

    #[test]
    fn t_args_with_preposition() {
        let i = parse_command("put coin in box");
        assert_eq!(i.args, vec!["put", "coin", "in", "box"]);
    }

    #[test]
    fn t_args_with_quotes() {
        let i = parse_command(r#"take "red card""#);
        assert_eq!(i.args, vec!["take", "red card"]);
    }

    // ---- Real-world scenarios ----

    #[test]
    fn t_scenario_keypad() {
        let i = parse_command("enter 4312 on keypad");
        assert_eq!(i.verb, Verb::Custom("enter".to_string()));
        assert_eq!(i.direct_raw.as_deref(), Some("4312"));
        assert_eq!(i.preposition, Some(Preposition::On));
        assert_eq!(i.target.unwrap().head, "keypad");
    }

    #[test]
    fn t_scenario_lockpick() {
        let i = parse_command("unlock door with lockpick");
        assert_eq!(i.verb, Verb::Unlock);
        assert_eq!(i.direct.unwrap().head, "door");
        assert_eq!(i.preposition, Some(Preposition::With));
        assert_eq!(i.instrument.unwrap().head, "lockpick");
    }

    #[test]
    fn t_scenario_container() {
        let i = parse_command("search old wooden chest");
        assert_eq!(i.verb, Verb::Search);
        let np = i.direct.unwrap();
        assert_eq!(np.raw, "old wooden chest");
        assert_eq!(np.head, "chest");
    }

    #[test]
    fn t_scenario_talk() {
        let i = parse_command("talk to old man");
        assert_eq!(i.verb, Verb::Talk);
        assert_eq!(i.preposition, Some(Preposition::To));
        assert_eq!(i.direct.as_ref().unwrap().raw, "old man");
    }

    #[test]
    fn t_scenario_throw() {
        let i = parse_command("throw rock at window");
        assert_eq!(i.verb, Verb::Custom("throw".to_string()));
        assert_eq!(i.direct.unwrap().head, "rock");
        assert_eq!(i.preposition, Some(Preposition::At));
        assert_eq!(i.target.unwrap().head, "window");
    }
}
