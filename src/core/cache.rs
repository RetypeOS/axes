//! # Cache
//!
//! This module provides utilities for caching data, including calculating validation data for cache
//! entries.

use anyhow::{Context, Result};
use std::io::Read;
use std::{fs, path::Path, time::SystemTime};

/// The length to truncate the hash to, in bytes. 16 bytes = 32 hex characters.
const HASH_TRUNCATE_LENGTH: usize = 16;
/// The buffer size for streaming I/O when hashing files, in bytes. 8KB.
const HASH_BUFFER_SIZE: usize = 8192;

// In src/core/cache.rs

/// Represents the validation metadata for a cache entry.
/// This layered approach allows for fast checks before resorting to hashing.
#[derive(Debug, PartialEq, Eq)]
pub struct CacheValidationData {
    /// The last modification timestamp of the source file (`axes.toml`).
    pub timestamp: SystemTime,
    /// The size of the source file in bytes.
    pub file_size: u64,
    /// The BLAKE3 content hash of the source file, truncated for brevity.
    pub content_hash: String,
}

/// Calculates the validation metadata for a given file path.
///
/// This function implements a multi-layered validation strategy for performance:
/// 1. Timestamp (modified time)
/// 2. File size
/// 3. Content Hash (blake3)
///
/// # Returns
///
/// A `Result` containing the `CacheValidationData` on success, or an error if the file
/// cannot be read or its metadata cannot be accessed.
///
/// # Errors
///
/// Returns an I/O error if the file cannot be read or its metadata cannot be accessed.
pub fn calculate_validation_data(path: &Path) -> Result<CacheValidationData> {
    // ROBUSTNESS: Add a log at the function entry with a clear name.
    log::trace!("Calculating validation data for '{}'", path.display());

    // Layer 1: Filesystem metadata (fast check).
    let metadata = fs::metadata(path)
        .with_context(|| format!("Failed to read metadata for file '{}'", path.display()))?;

    let timestamp = metadata
        .modified()
        .context("Filesystem does not support modification timestamps.")?;
    let file_size = metadata.len();

    // Layer 2: Content Hash (definitive check).
    // Use streaming hashing to handle large files efficiently without
    // loading them entirely into memory.
    let mut file = fs::File::open(path)
        .with_context(|| format!("Failed to open file for hashing '{}'", path.display()))?;

    let mut hasher = blake3::Hasher::new();

    // Use a buffer to read the file in chunks.
    // 8KB is a common and efficient buffer size for I/O.
    let mut buffer = [0; HASH_BUFFER_SIZE];
    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(
            buffer
                .get(..bytes_read)
                .expect("bytes_read should be within buffer bounds"),
        );
    }

    let hash = hasher.finalize();
    let content_hash = hex::encode(&hash.as_bytes()[..HASH_TRUNCATE_LENGTH]);

    log::debug!(
        "Validation data for '{}': size={}, hash={}",
        path.display(),
        file_size,
        content_hash
    );

    Ok(CacheValidationData {
        timestamp,
        file_size,
        content_hash,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_calculate_validation_data_success() {
        // --- Setup ---
        let content = b"hello world"; // Use a byte string literal
        let mut temp_file = NamedTempFile::new().unwrap();

        // Use `write_all` to write the exact bytes without any translation.
        temp_file.write_all(content).unwrap();
        temp_file.flush().unwrap(); // Ensure data is written to disk before reading metadata

        // --- Execute ---
        let result = calculate_validation_data(temp_file.path());

        // --- Assert ---
        assert!(result.is_ok());
        let data = result.unwrap();

        assert_eq!(data.file_size, 11);

        // Pre-calculated blake3 hash of the bytes "hello world", truncated to 16 bytes.
        // This hash is now platform-independent and correct.
        let expected_hash = "d74981efa70a0c880b8d8c1985d075db";

        assert_eq!(data.content_hash, expected_hash);

        // Timestamp check (can be brittle, just check it's recent)
        let now = SystemTime::now();
        let difference = now.duration_since(data.timestamp).unwrap();
        assert!(difference.as_secs() < 5);
    }

    #[test]
    fn test_calculate_validation_data_file_not_found() {
        // --- Setup ---
        let non_existent_path = Path::new("non_existent_file_for_test.tmp");

        // --- Execute ---
        let result = calculate_validation_data(non_existent_path);

        // --- Assert ---
        assert!(result.is_err());
    }
}
