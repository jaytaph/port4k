use crate::models::account::Account;
use crate::models::room::RoomView;
use crate::models::types::{AccountId, RoomId, ZoneId};
use crate::models::zone::ZoneContext;
use serde::Serialize;
use serde::ser::SerializeStruct;

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
    pub zone_id: ZoneId,
    pub room_id: RoomId,
    pub account_id: AccountId,
    pub zone_ctx: ZoneContext,
    pub room_view: RoomView,
}

impl Serialize for Cursor {
    // We don't want to serialize the entire ZoneContext and RoomView (too much data)
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("Cursor", 3)?;
        state.serialize_field("zone_ctx.zone.key", &self.zone_ctx.zone.key)?;
        state.serialize_field("zone_ctx.blueprint.key", &self.zone_ctx.blueprint.key)?;
        state.serialize_field("zone_id", &self.zone_id)?;
        state.serialize_field("room_id", &self.room_id)?;
        state.serialize_field("account_id", &self.account_id)?;
        state.end()
    }
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

    // Are we currently in the lua repl?
    pub in_lua_repl: bool,

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
            in_lua_repl: false,
        }
    }
}
