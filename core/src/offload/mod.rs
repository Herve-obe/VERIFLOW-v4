//! OFFLOAD : copie sécurisée source vers N destinations, checksums, ASC MHL v2 (charte §7.1).

pub mod engine;
pub mod existing;
pub mod hash;
mod io;
pub mod job;
pub mod mhl;
pub mod mhl_read;
pub mod mhl_verify;
pub mod report;
pub mod scan;
pub mod schedule;
pub mod storage;
