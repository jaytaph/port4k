use port4k_core::Username;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnState {
    PreLogin,
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
    Live {
        room_id: i64,
    },
    Playtest {
        bp: String,
        room: String,
        prev_room_id: Option<i64>,
    },
}

#[derive(Debug)]
pub struct Session {
    pub name: Option<Username>,
    pub state: ConnState,
    pub world: Option<WorldMode>,
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
