//! YASR — Yet Another Statusline in Rust, for Claude Code sessions
//!
//! Library mode (used by CCB):  cargo build --features status
//! Standalone binary:           cargo build --features bin
//!
//! When `status` feature is enabled, CCB gets the statusline renderer.
//! When disabled, CCB builds fine without it — no external dependency needed.

pub mod border;
pub mod ccb_bridge;
pub mod demo;
pub mod gradient;
pub mod mon;
pub mod renderer;
pub mod session;
pub mod themes;

pub use renderer::render;
pub use session::SessionInfo;
pub use themes::{resolve_theme, Theme};

// Re-export CCB bridge types for convenience
pub use ccb_bridge::{build_session_info, StatusInput};
