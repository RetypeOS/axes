// src/lib.rs

include!(concat!(env!("OUT_DIR"), "/translations.rs"));

pub mod cli;
pub mod constants;
pub mod core;
pub mod models;
pub mod system;
