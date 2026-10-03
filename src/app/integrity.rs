// CS-30003: Reliable Data Transfer over UDP
// Author: Sahan Maiti (Team Lead)
// Component: Application Layer - SHA-256 End-to-End Integrity Verification

use std::fmt::Write as FmtWrite;
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;
use sha2::{Digest, Sha256};

/// Summary of an end-to-end file integrity verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegrityReport {
    pub source_path: String,
    pub received_path: String,
    pub source_sha256: String,
    pub received_sha256: String,
    pub is_match: bool,
    pub total_bytes: u64,
}

/// Computes the SHA-256 digest of an in-memory byte slice, returned as a 64-character lowercase hex string.
pub fn compute_sha256(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    format_hex(&result)
}

/// Computes the SHA-256 digest of a file on disk by streaming in 64 KB buffers.
pub fn compute_file_sha256<P: AsRef<Path>>(path: P) -> io::Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 65536];

    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    let result = hasher.finalize();
    Ok(format_hex(&result))
}

/// Verifies whether two files on disk are byte-for-byte identical using SHA-256.
pub fn verify_file_integrity<P1: AsRef<Path>, P2: AsRef<Path>>(
    source_path: P1,
    received_path: P2,
) -> io::Result<IntegrityReport> {
    let src_p = source_path.as_ref();
    let recv_p = received_path.as_ref();

    let src_meta = std::fs::metadata(src_p)?;
    let recv_meta = std::fs::metadata(recv_p)?;

    let src_hash = compute_file_sha256(src_p)?;
    let recv_hash = compute_file_sha256(recv_p)?;

    let is_match = (src_meta.len() == recv_meta.len()) && (src_hash == recv_hash);

    Ok(IntegrityReport {
        source_path: src_p.to_string_lossy().to_string(),
        received_path: recv_p.to_string_lossy().to_string(),
        source_sha256: src_hash,
        received_sha256: recv_hash,
        is_match,
        total_bytes: recv_meta.len(),
    })
}

/// Formats a byte slice into a lowercase hexadecimal string.
fn format_hex(bytes: &[u8]) -> String {
    let mut hex = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(hex, "{:02x}", b);
    }
    hex
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sha256_known_vector() {
        // Standard NIST test vector: sha256("") = e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
        let empty_hash = compute_sha256(b"");
        assert_eq!(
            empty_hash,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );

        // sha256("abc") = ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad
        let abc_hash = compute_sha256(b"abc");
        assert_eq!(
            abc_hash,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn test_format_hex() {
        let sample = [0x01, 0x0f, 0xa5, 0xff];
        assert_eq!(format_hex(&sample), "010fa5ff");
    }
}
