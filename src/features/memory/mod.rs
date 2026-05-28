//! Memory — Pattern Mining & Skill Generation
//!
//! Sub-modules:
//!   - `db` — mined_patterns table schema and CRUD
//!   - `mine` — SQL pattern detection queries
//!   - `skills` — skill file generation and index rebuild
//!   - `patterns` — list and suppress helpers

pub mod db;
pub mod mine;
pub mod patterns;
pub mod skills;