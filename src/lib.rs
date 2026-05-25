mod analytics;
mod cli;
mod config;
mod log;
mod utils;

pub mod features {
    pub mod buzz;
    pub mod context;
    pub mod cut;
    #[cfg(feature = "expert")]
    pub mod expert;
    #[cfg(feature = "classify")]
    pub mod classify;
    #[cfg(feature = "fade")]
    pub mod fade;
    #[cfg(feature = "graph")]
    pub mod graph;
    #[cfg(feature = "route")]
    pub mod route;
    pub mod index;
    pub mod lineup;
    #[cfg(feature = "trim")]
    pub mod trim;
    pub mod install;
}

pub use cli::{Cli, Command, StyleCmd};