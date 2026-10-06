//! Cœur métier de VERIFLOW.
//!
//! Ce crate ne dépend d'aucune interface graphique : il peut être testé seul
//! et réutilisé par une autre interface (charte §5.5).

pub mod error;
pub mod explorer;
pub mod media;
pub mod offload;
pub mod player;
pub mod project;
pub mod report;
pub mod sync;
pub mod tools;
pub mod transcode;

pub use error::{Error, Result};

/// Chemin absolu, sans accès au disque. Sous Windows, corrige aussi les chemins
/// relatifs à un lecteur (« D:EXPORT ») et les barres « / » mélangées, que
/// l'Explorateur Windows refuse d'ouvrir.
pub fn absolute_path(path: &std::path::Path) -> std::path::PathBuf {
    use std::path::Component;
    let mut p = path.to_path_buf();
    // Lecteur seul (« D: ») : sa racine, pas son dossier courant.
    if matches!(
        p.components().collect::<Vec<_>>()[..],
        [Component::Prefix(_)]
    ) {
        p.push(std::path::MAIN_SEPARATOR_STR);
    }
    std::path::absolute(&p).unwrap_or(p)
}

/// Version de VERIFLOW, reprise du manifeste Cargo.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
