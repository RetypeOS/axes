// src/lib.rs

include!(concat!(env!("OUT_DIR"), "/translations.rs"));

use std::sync::atomic::AtomicBool;
use std::sync::Arc;
pub type CancellationToken = Arc<AtomicBool>;

pub mod cli;
pub mod constants;
pub mod core;
pub mod models;
pub mod system;
