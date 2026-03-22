/// Shared SHA-256 helpers used by both build.rs and checksum trait.
/// Included via `include!("sha256.rs")` to avoid duplication.

/// SHA-256 hex digest of raw bytes.
fn sha256_bytes(input: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input);
    format!("{:x}", hasher.finalize())
}

/// SHA-256 hex digest of a string.
#[allow(dead_code)]
fn sha256_hex(input: &str) -> String {
    sha256_bytes(input.as_bytes())
}
