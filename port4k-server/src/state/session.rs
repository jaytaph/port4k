use crate::models::account::Account;
use crate::models::room::RoomView;
use crate::models::types::RoomId;
use crate::models::zone::ZoneContext;

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
    pub zone_ctx: ZoneContext,
    pub room_id: RoomId,
    pub room_view: RoomView,
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

    // Which map am I?
    pub zone_ctx: Option<ZoneContext>,

    // Where am I (on the map)?
    pub cursor: Option<Cursor>,
    // Previous cursors (for "back" command)
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
            zone_ctx: None,
            prev_cursors: Vec::new(),
            tty_cols: None,
            tty_rows: None,
        }
    }
}
