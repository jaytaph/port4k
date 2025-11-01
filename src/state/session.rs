use crate::models::account::Account;
use crate::models::realm::Realm;
use crate::models::room::RoomView;
use crate::models::types::{AccountId, RealmId, RoomId};
use std::sync::Arc;

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
    /// Realm information
    pub realm_id: RealmId,
    pub realm: Arc<Realm>,

    /// Room information
    pub room_id: RoomId,
    pub room: Arc<RoomView>,

    /// Account information
    pub account_id: AccountId,
    pub account: Arc<Account>,
}

impl Cursor {
    pub(crate) fn new(realm: Realm, room: RoomView, account: Account) -> Self {
        Self {
            realm_id: realm.id,
            realm: Arc::new(realm),
            room_id: room.blueprint.id,
            room: Arc::new(room),
            account_id: account.id,
            account: Arc::new(account),
        }
    }
}

#[derive(Debug)]
pub struct Session {
    // When is the session started/created
    pub session_started: std::time::Instant,

    /// Protocol used by the client
    #[allow(unused)]
    protocol: Protocol,
    /// User Account (if logged in)
    account: Option<Arc<Account>>,
    /// Current connection state
    state: ConnState,

    // Are we currently in the lua repl?
    in_lua_repl: bool,

    // Where am I (on the map)?
    cursor: Option<Cursor>,
    // Previous cursors (for "back" command)
    prev_cursors: Vec<Cursor>,

    // Terminal size (if known)
    tty_cols: Option<usize>,
    tty_rows: Option<usize>,
}

impl Session {
    pub fn new(protocol: Protocol) -> Self {
        Self {
            session_started: std::time::Instant::now(),
            protocol,
            account: None,
            state: ConnState::PreLogin,
            cursor: None,
            prev_cursors: Vec::new(),
            tty_cols: None,
            tty_rows: None,
            in_lua_repl: false,
        }
    }

    pub fn is_logged_in(&self) -> bool {
        self.state == ConnState::LoggedIn && self.account.is_some()
    }

    pub fn get_account(&self) -> Option<Arc<Account>> {
        self.account.clone()
    }

    pub fn get_cursor(&self) -> Option<Cursor> {
        self.cursor.clone()
    }

    pub fn has_cursor(&self) -> bool {
        self.cursor.is_some()
    }

    pub fn set_cursor(&mut self, cursor: Option<Cursor>) {
        if let Some(c) = &self.cursor {
            self.prev_cursors.push(c.clone());
        }
        self.cursor = cursor;
    }

    pub fn login(&mut self, account: Account, realm: Realm, room: RoomView) {
        let acc = Arc::new(account);
        self.account = Some(acc.clone());
        self.state = ConnState::LoggedIn;
        self.cursor = Some(Cursor::new(realm, room, (*acc).clone()));
    }

    pub fn logout(&mut self) {
        self.account = None;
        self.state = ConnState::PreLogin;
        self.cursor = None;
        self.prev_cursors.clear();
    }

    pub fn in_lua(&mut self, in_repl: bool) {
        self.in_lua_repl = in_repl;
    }
    pub fn is_in_lua(&self) -> bool {
        self.in_lua_repl
    }

    pub fn set_tty(&mut self, cols: usize, rows: usize) {
        self.tty_cols = Some(cols);
        self.tty_rows = Some(rows);
    }

    pub fn get_tty(&self) -> Option<(usize, usize)> {
        match (self.tty_cols, self.tty_rows) {
            (Some(c), Some(r)) => Some((c, r)),
            _ => None,
        }
    }
}
