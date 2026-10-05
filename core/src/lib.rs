//! Cœur métier de VERIFLOW.
//!
//! Ce crate ne dépend d'aucune interface graphique : il peut être testé seul
//! et réutilisé par une autre interface (charte §5.5).

pub mod error;
pub mod media;
pub mod offload;
pub mod player;
pub mod project;
pub mod report;
pub mod sync;
pub mod transcode;

pub use error::{Error, Result};

/// Version de VERIFLOW, reprise du manifeste Cargo.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
