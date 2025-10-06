// src/core/cache.rs (New File)

use anyhow::{Context, Result};
use log::debug;
use std::{
    fs,
    path::Path,
    time::{SystemTime},
};

const HASH_TRUNCATE_LENGTH: usize = 16; // 16 bytes = 32 hex characters

/// Represents the validation metadata for a cache entry.
/// This layered approach allows for fast checks before resorting to hashing.
#[derive(Debug, PartialEq, Eq)]
pub struct CacheValidationData {
    pub timestamp: SystemTime,
    pub file_size: u64,
    pub content_hash: String,
}

/// Calculates the validation metadata for a given file path.
///
/// This function implements a multi-layered validation strategy for performance:
/// 1. Timestamp (modified time)
/// 2. File size
/// 3. Content Hash (blake3)
///
/// # Errors
/// Returns an I/O error if the file cannot be read or its metadata cannot be accessed.
pub fn calculate_validation_data(path: &Path) -> Result<CacheValidationData> {
    debug!("Calculating validation data for '{}'", path.display());
    
    // Layer 0 & 1: Timestamp and file size (fast metadata check)
    let metadata = fs::metadata(path)
        .with_context(|| format!("Failed to read metadata for file '{}'", path.display()))?;
    
    let timestamp = metadata.modified()?;
    let file_size = metadata.len();
    
    // Layer 2: Content Hash (definitive check)
    let content = fs::read(path)
        .with_context(|| format!("Failed to read content of file '{}'", path.display()))?;
    
    let hash = blake3::hash(&content);
    let content_hash = hex::encode(&hash.as_bytes()[..HASH_TRUNCATE_LENGTH]);
    
    debug!(
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
    use std::io::Write; // Keep `Write` for the `write_all` method
    use tempfile::NamedTempFile;

    #[test]
    fn test_calculate_validation_data_success() {
        // --- Setup ---
        let content = b"hello world"; // Use a byte string literal
        let mut temp_file = NamedTempFile::new().unwrap();
        
        // FIX: Use `write_all` to write the exact bytes without any translation.
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
        
        // FIX: Update the expected hash to the correct one calculated from raw bytes.
        // Your machine's calculation was the correct one!
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