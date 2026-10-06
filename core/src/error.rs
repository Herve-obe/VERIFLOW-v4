//! Erreurs communes du cœur.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("erreur d'entrée/sortie : {0}")]
    Io(#[from] std::io::Error),

    #[error("erreur de base de données : {0}")]
    Database(#[from] rusqlite::Error),

    #[error("le fichier existe déjà : {0}")]
    AlreadyExists(String),

    #[error("fichier projet introuvable : {0}")]
    NotFound(String),

    #[error("ce fichier n'est pas un projet VERIFLOW : {0}")]
    NotAProject(String),

    #[error(
        "outil introuvable : {0}. Installe FFmpeg ou indique son dossier dans VERIFLOW_FFMPEG_DIR"
    )]
    ToolMissing(String),

    #[error("échec de {tool} : {message}")]
    Tool { tool: String, message: String },

    #[error("opération annulée")]
    Cancelled,

    #[error("audio : {0}")]
    Audio(String),

    #[error("média non pris en charge : {0}")]
    Unsupported(String),

    #[error("projet créé par une version plus récente de VERIFLOW (schéma {found}, maximum supporté {supported})")]
    SchemaTooNew { found: i64, supported: i64 },
}

pub type Result<T> = std::result::Result<T, Error>;
