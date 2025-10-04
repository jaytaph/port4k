use crate::db::models::account::Account;
use crate::db::types::{BlueprintId, RoomId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnState {
    /// User is not logged in
    PreLogin,
    /// User is logged in
    LoggedIn,
}

#[derive(Debug)]
pub struct Editor {
    pub bp: BlueprintId,
    pub room: RoomId,
    pub event: String,
    pub buf: String,
}

#[derive(Debug, Clone)]
pub enum WorldMode {
    /// Live world available for everyone
    Live {
        /// Current room ID
        room_id: RoomId,
    },
    /// Playtest world, private to the user
    Playtest {
        /// Blueprint Id
        bp: BlueprintId,
        /// Current room
        room: RoomId,
        /// Previous room ID, if any
        prev_room_id: Option<RoomId>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    Telnet,
    WebSocket,
    // SSH (not implemented yet)
}

#[derive(Debug)]
pub struct Session {
    /// Protocol used by the client
    pub protocol: Protocol,
    /// User Account (if logged in)
    pub account: Option<Account>,
    /// Current connection state
    pub state: ConnState,
    /// Current world mode, if any
    pub world: Option<WorldMode>,
    /// Current editor state, if any
    pub editor: Option<Editor>,

    // Terminal size (if known)
    pub tty_cols: Option<usize>,
    pub tty_rows: Option<usize>,
}

impl Session {
    pub fn new(protocol: Protocol) -> Self {
        Self {
            protocol,
            account: None,
            state: ConnState::PreLogin,
            world: None,
            editor: None,
            tty_cols: None,
            tty_rows: None,
        }
    }
}
