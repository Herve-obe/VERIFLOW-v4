//! Exports vers le montage (FCPXML, XMEML, EDL, ALE, OTIO, CSV Resolve) depuis
//! MEDIA : des clips de chutier, sans séquence (sauf l'EDL). Ce module porte
//! le modèle commun et les chemins ; chaque format a son fichier.

pub mod fcpxml;
pub mod model;
pub mod paths;

pub use model::{EditClip, EditFormat, EditOptions, FcpxmlVersion, PathMode, SyncedSound};
