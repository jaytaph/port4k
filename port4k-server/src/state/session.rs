use crate::db::models::account::Account;
use crate::db::models::blueprint::Blueprint;
use crate::db::models::room::RoomView;
use crate::db::models::zone::{Zone, ZoneKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnState {
    /// User is not logged in
    PreLogin,
    /// User is logged in
    LoggedIn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    Telnet,
    WebSocket,
    // SSH (not implemented yet)
}

#[derive(Debug, Clone)]
pub struct Cursor {
    pub zone: Zone,
    pub zone_kind: ZoneKind,
    pub bp: Blueprint,
    pub room: RoomView,
}

#[derive(Debug)]
pub struct Session {
    // When is the session started/created
    pub session_started: std::time::Instant,

    /// Protocol used by the client
    pub protocol: Protocol,
    /// User Account (if logged in)
    pub account: Option<Account>,
    /// Current connection state
    pub state: ConnState,
    /// Current world cursor (where am i?)
    pub cursor: Option<Cursor>,
    /// Previous cursors (for backtracking)
    pub prev_cursors: Vec<Cursor>,
    // Terminal size (if known)
    pub tty_cols: Option<usize>,
    pub tty_rows: Option<usize>,
}

impl Session {
    pub fn new(protocol: Protocol) -> Self {
        Self {
            session_started: std::time::Instant::now(),
            protocol,
            account: None,
            state: ConnState::PreLogin,
            cursor: None,
            tty_cols: None,
            tty_rows: None,
        }
    }
}
