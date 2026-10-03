// CS-30003: Reliable Data Transfer over UDP
// Author: Sahan Maiti (Team Lead)
// Integration Tests: File Chunker, Reassembler & SHA-256 Verification

use reliable_udp::app::{compute_sha256, Chunker, Reassembler};

#[test]
fn test_end_to_end_chunk_and_reassemble() {
    // Generate synthetic payload (5000 bytes)
    let original_data: Vec<u8> = (0..5000).map(|i| (i % 256) as u8).collect();
    let original_hash = compute_sha256(&original_data);

    let chunker = Chunker::new(1400);
    let packets = chunker.chunk_bytes(&original_data);

    // 5000 bytes with 1400 MTU chunks = 4 chunks (1400 + 1400 + 1400 + 800)
    assert_eq!(packets.len(), 4);

    let mut reassembler = Reassembler::new();
    for pkt in packets {
        reassembler.push_chunk(pkt.seq_num, &pkt.payload);
    }

    assert!(reassembler.is_complete(4));
    assert_eq!(reassembler.reconstructed_data, original_data);
    assert_eq!(reassembler.sha256_digest(), original_hash);
}

#[test]
fn test_out_of_order_reassembly_selective_repeat_model() {
    let original_data: Vec<u8> = b"Reliable Data Transfer over UDP - CS-30003".to_vec();
    let original_hash = compute_sha256(&original_data);

    let chunker = Chunker::new(10);
    let packets = chunker.chunk_bytes(&original_data);
    let total_chunks = packets.len() as u32;

    // Simulate reversed arrival order (packets arrive backwards)
    let mut reassembler = Reassembler::new();
    for pkt in packets.iter().rev() {
        reassembler.push_chunk(pkt.seq_num, &pkt.payload);
    }

    assert!(reassembler.is_complete(total_chunks));
    assert_eq!(reassembler.reconstructed_data, original_data);
    assert_eq!(reassembler.sha256_digest(), original_hash);
}
