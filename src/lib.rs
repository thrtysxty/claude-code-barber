mod analytics;
mod cli;
mod config;
mod log;
mod utils;

pub mod features {
    pub mod buzz;
    #[cfg(feature = "classify")]
    pub mod classify;
    pub mod context;
    pub mod cut;
    #[cfg(feature = "expert")]
    pub mod expert;
    #[cfg(feature = "fade")]
    pub mod fade;
    #[cfg(feature = "graph")]
    pub mod graph;
    pub mod index;
    pub mod install;
    pub mod lineup;
    #[cfg(feature = "route")]
    pub mod route;
    #[cfg(feature = "trim")]
    pub mod trim;
}

pub use cli::{Cli, Command, StyleCmd};
