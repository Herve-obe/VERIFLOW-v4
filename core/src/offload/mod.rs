//! OFFLOAD : copie sécurisée source vers N destinations, checksums, ASC MHL v2 (charte §7.1).

pub mod engine;
pub mod hash;
mod io;
pub mod job;
pub mod mhl;
pub mod report;
pub mod scan;
