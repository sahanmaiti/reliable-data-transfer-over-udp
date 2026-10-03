// CS-30003: Reliable Data Transfer over UDP
// Author: Sahan Maiti (Team Lead)
// Component: Application Layer - File Chunking, Buffering & Reconstruction

use crate::packet::{Packet, MAX_PAYLOAD_SIZE};
use crate::app::integrity::compute_sha256;
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;

/// Chunks an arbitrary input file into a sequence of sequence-numbered DATA packets.
pub struct Chunker {
    chunk_size: usize,
}

impl Default for Chunker {
    fn default() -> Self {
        Self::new(MAX_PAYLOAD_SIZE)
    }
}

impl Chunker {
    /// Creates a new Chunker with the specified maximum payload chunk size.
    pub fn new(chunk_size: usize) -> Self {
        assert!(chunk_size > 0, "Chunk size must be non-zero");
        assert!(chunk_size <= MAX_PAYLOAD_SIZE, "Chunk size cannot exceed MAX_PAYLOAD_SIZE");
        Chunker { chunk_size }
    }

    /// Reads raw bytes and splits them into DATA packets with 0-indexed sequence numbers.
    pub fn chunk_bytes(&self, data: &[u8]) -> Vec<Packet> {
        if data.is_empty() {
            // Represent an empty file as a single empty DATA packet
            return vec![Packet::new_data(0, Vec::new())];
        }

        data.chunks(self.chunk_size)
            .enumerate()
            .map(|(seq_num, chunk)| Packet::new_data(seq_num as u32, chunk.to_vec()))
            .collect()
    }

    /// Reads a file from disk and decomposes it into DATA packets.
    pub fn chunk_file<P: AsRef<Path>>(&self, path: P) -> io::Result<Vec<Packet>> {
        let mut file = File::open(path)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;
        Ok(self.chunk_bytes(&buffer))
    }
}

/// Buffers received chunks and writes them in strict sequence order to reconstruct the source file.
pub struct Reassembler {
    /// Next expected consecutive sequence number for in-order delivery.
    pub next_expected_seq: u32,
    /// Out-of-order buffer (stores future chunks in Selective Repeat until sequence gap is filled).
    pub buffer: BTreeMap<u32, Vec<u8>>,
    /// Continuous reconstructed byte stream.
    pub reconstructed_data: Vec<u8>,
    /// Total unique payload bytes accepted.
    pub total_bytes_accepted: usize,
    /// Duplicate chunks ignored.
    pub duplicate_chunks_count: usize,
}

impl Default for Reassembler {
    fn default() -> Self {
        Self::new()
    }
}

impl Reassembler {
    pub fn new() -> Self {
        Reassembler {
            next_expected_seq: 0,
            buffer: BTreeMap::new(),
            reconstructed_data: Vec::new(),
            total_bytes_accepted: 0,
            duplicate_chunks_count: 0,
        }
    }

    /// Ingests a received DATA chunk with sequence number `seq_num`.
    /// Returns `true` if the chunk advanced the in-order delivery window.
    pub fn push_chunk(&mut self, seq_num: u32, payload: &[u8]) -> bool {
        // If this sequence number was already delivered, discard as duplicate
        if seq_num < self.next_expected_seq {
            self.duplicate_chunks_count += 1;
            return false;
        }

        // Buffer the chunk (or overwrite duplicate if already buffered)
        if self.buffer.contains_key(&seq_num) {
            self.duplicate_chunks_count += 1;
            return false;
        }

        self.buffer.insert(seq_num, payload.to_vec());

        // Drain consecutive chunks from the buffer starting at `next_expected_seq`
        let mut advanced = false;
        while let Some(chunk) = self.buffer.remove(&self.next_expected_seq) {
            self.total_bytes_accepted += chunk.len();
            self.reconstructed_data.extend_from_slice(&chunk);
            self.next_expected_seq += 1;
            advanced = true;
        }

        advanced
    }

    /// Checks if all sequence numbers up to `total_chunks - 1` have been reconstructed.
    pub fn is_complete(&self, total_chunks: u32) -> bool {
        self.next_expected_seq >= total_chunks && self.buffer.is_empty()
    }

    /// Computes the SHA-256 hash of the fully reconstructed payload stream.
    pub fn sha256_digest(&self) -> String {
        compute_sha256(&self.reconstructed_data)
    }

    /// Flushes the reconstructed payload to disk at the specified output path.
    pub fn write_to_disk<P: AsRef<Path>>(&self, output_path: P) -> io::Result<usize> {
        let mut file = File::create(output_path)?;
        file.write_all(&self.reconstructed_data)?;
        file.flush()?;
        Ok(self.reconstructed_data.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunker_basic() {
        let data = b"ABCDEFGHIJ"; // 10 bytes
        let chunker = Chunker::new(3); // 3 bytes per chunk
        let packets = chunker.chunk_bytes(data);

        assert_eq!(packets.len(), 4); // [3, 3, 3, 1]
        assert_eq!(packets[0].seq_num, 0);
        assert_eq!(packets[0].payload, b"ABC");
        assert_eq!(packets[1].seq_num, 1);
        assert_eq!(packets[1].payload, b"DEF");
        assert_eq!(packets[2].seq_num, 2);
        assert_eq!(packets[2].payload, b"GHI");
        assert_eq!(packets[3].seq_num, 3);
        assert_eq!(packets[3].payload, b"J");
    }

    #[test]
    fn test_reassembler_in_order() {
        let mut reassembler = Reassembler::new();
        assert!(reassembler.push_chunk(0, b"Hello, "));
        assert!(reassembler.push_chunk(1, b"World!"));

        assert_eq!(reassembler.next_expected_seq, 2);
        assert_eq!(reassembler.reconstructed_data, b"Hello, World!");
        assert_eq!(reassembler.duplicate_chunks_count, 0);
    }

    #[test]
    fn test_reassembler_out_of_order() {
        let mut reassembler = Reassembler::new();

        // Chunks arrive out of order: 2, 0, 1
        assert!(!reassembler.push_chunk(2, b"Selective"));
        assert_eq!(reassembler.next_expected_seq, 0); // Gap at 0

        assert!(reassembler.push_chunk(0, b"Testing "));
        assert_eq!(reassembler.next_expected_seq, 1); // Delivered 0, waiting for 1

        assert!(reassembler.push_chunk(1, b"out-of-order "));
        assert_eq!(reassembler.next_expected_seq, 3); // Now 0, 1, 2 are all delivered!

        assert_eq!(
            reassembler.reconstructed_data,
            b"Testing out-of-order Selective"
        );
    }

    #[test]
    fn test_reassembler_duplicate_rejection() {
        let mut reassembler = Reassembler::new();
        assert!(reassembler.push_chunk(0, b"Data0"));
        assert!(!reassembler.push_chunk(0, b"Data0")); // Duplicate past
        assert_eq!(reassembler.duplicate_chunks_count, 1);

        assert!(!reassembler.push_chunk(2, b"Data2")); // Buffer future
        assert!(!reassembler.push_chunk(2, b"Data2")); // Duplicate future
        assert_eq!(reassembler.duplicate_chunks_count, 2);
    }
}
