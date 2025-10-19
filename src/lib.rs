include!(concat!(env!("OUT_DIR"), "/translations.rs"));

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
pub type CancellationToken = Arc<AtomicBool>;

pub mod cli;
pub mod constants;
pub mod core;
pub mod models;
pub mod system;
