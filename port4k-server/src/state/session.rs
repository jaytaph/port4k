use port4k_core::Username;
use crate::db::types::RoomId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnState {
    /// User is not logged in
    PreLogin,
    /// User is logged in
    LoggedIn,
}

#[derive(Debug)]
pub struct Editor {
    /// Blueprint Id
    pub bp: String,
    /// Room id
    pub room: String,
    /// Event name
    pub event: String,
    /// Input buffer
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
        bp: String,
        /// Current room name
        room: String,
        /// Previous room ID, if any
        prev_room_id: Option<RoomId>,
    },
}

#[derive(Debug)]
pub struct Session {
    /// Name of the user currently logged in (or None when not logged in)
    pub name: Option<Username>,
    /// Current connection state
    pub state: ConnState,
    /// Current world mode, if any
    pub world: Option<WorldMode>,
    /// Current editor state, if any
    pub editor: Option<Editor>,
}

impl Default for Session {
    fn default() -> Self {
        Self {
            name: None,
            state: ConnState::PreLogin,
            world: None,
            editor: None,
        }
    }
}
