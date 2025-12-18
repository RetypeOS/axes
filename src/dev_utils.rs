// src/dev_utils.rs
#![allow(dead_code)] // Allow unused code, as this is a temporary debug utility

use std::time::Instant;

/// A simple RAII timer for profiling blocks of code.
/// When created, it records the start time. When it goes out of scope (is dropped),
/// it calculates the elapsed time and prints it to the console.
#[derive(Debug)]
pub struct BlockTimer {
    name: String,
    start: Instant,
}

impl BlockTimer {
    /// Creates a new timer and starts it immediately.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            start: Instant::now(),
        }
    }
}

impl Drop for BlockTimer {
    fn drop(&mut self) {
        let elapsed = self.start.elapsed();
        // Print in a structured format with microseconds for high precision.
        println!(
            "PROFILE [{}]: {} µs",
            self.name,
            elapsed.as_micros()
        );
    }
}