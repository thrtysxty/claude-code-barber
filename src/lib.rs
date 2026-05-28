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
    #[cfg(feature = "factory")]
    pub mod factory;
    #[cfg(feature = "fade")]
    pub mod fade;
    #[cfg(feature = "graph")]
    pub mod graph;
    pub mod index;
    pub mod install;
    pub mod lineup;
    #[cfg(feature = "loop")]
    pub mod loop_cmd;
    pub mod model_metadata;
    #[cfg(feature = "route")]
    pub mod providers;
    #[cfg(feature = "status")]
    pub mod rates;
    #[cfg(feature = "route")]
    pub mod route;
    #[cfg(feature = "status")]
    pub mod status;
    #[cfg(feature = "trim")]
    pub mod trim;
}
