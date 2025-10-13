pub mod ansi;
pub mod banner;
pub mod commands;
pub mod config;
pub mod db;
pub mod error;
pub mod hardening;
pub mod import;
pub mod input;
pub mod lua;
pub mod models;
pub mod net;
pub mod renderer;
pub mod services;
pub mod state;
pub mod util;

// Convenient re-exports (so call sites can do `port4k_server::Registry`, etc.)
pub use commands::process_command;
pub use state::{
    registry::Registry,
    session::{ConnState, Session},
};
