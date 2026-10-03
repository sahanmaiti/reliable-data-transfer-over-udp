// CS-30003: Reliable Data Transfer over UDP
// Author: Soumyadeb Mukherjee
// Unit Tests: Stop-and-Wait, Go-Back-N, and Selective Repeat State Machines

use reliable_udp::arq::{
    GoBackNReceiver, GoBackNSender, SelectiveRepeatReceiver, SelectiveRepeatSender,
    StopAndWaitReceiver, StopAndWaitSender,
};

#[test]
fn test_stop_and_wait_clean_transfer() {
    let mut sender = StopAndWaitSender::new(5);
    let mut receiver = StopAndWaitReceiver::new();

    let chunks = vec![b"chunk_0".to_vec(), b"chunk_1".to_vec(), b"chunk_2".to_vec()];

    for chunk in chunks.clone() {
        assert!(sender.can_send());
        let data_pkt = sender.send_chunk(chunk).unwrap();
        assert!(!sender.can_send()); // S&W window full

        // Receiver receives DATA and returns ACK
        let ack_pkt = receiver.handle_packet(&data_pkt).expect("Receiver must ACK");
        let acked = sender.handle_ack(&ack_pkt);
        assert!(acked);
    }

    assert_eq!(receiver.delivered_chunks(), &chunks[..]);
    assert_eq!(sender.retransmissions, 0);
}

#[test]
fn test_stop_and_wait_lost_packet_timeout_recovery() {
    let mut sender = StopAndWaitSender::new(5);
    let mut receiver = StopAndWaitReceiver::new();

    let data_pkt = sender.send_chunk(b"hello".to_vec()).unwrap();

    // Simulate LOST packet: receiver never receives data_pkt
    // Sender times out!
    let retry_pkt = sender.handle_timeout().expect("Must retransmit on timeout");
    assert_eq!(sender.retransmissions, 1);
    assert_eq!(retry_pkt.seq_num, data_pkt.seq_num);

    // Receiver now receives the retransmission
    let ack_pkt = receiver.handle_packet(&retry_pkt).unwrap();
    assert!(sender.handle_ack(&ack_pkt));
    assert_eq!(receiver.delivered_chunks().len(), 1);
}

#[test]
fn test_stop_and_wait_duplicate_handling_on_lost_ack() {
    let mut sender = StopAndWaitSender::new(5);
    let mut receiver = StopAndWaitReceiver::new();

    let data_pkt = sender.send_chunk(b"payload".to_vec()).unwrap();
    let _ack_pkt = receiver.handle_packet(&data_pkt).unwrap();

    // Simulate LOST ACK: sender never gets ACK, times out
    let retry_pkt = sender.handle_timeout().unwrap();

    // Receiver receives duplicate DATA
    let dup_ack = receiver.handle_packet(&retry_pkt).expect("Must re-ACK duplicate");
    assert_eq!(receiver.duplicates_count, 1);
    // Crucial invariant: duplicate packet must NOT be delivered twice to application!
    assert_eq!(receiver.delivered_chunks().len(), 1);

    // Sender processes duplicate ACK and advances
    assert!(sender.handle_ack(&dup_ack));
    assert!(sender.can_send());
}

#[test]
fn test_gobackn_window_filling_and_cumulative_ack() {
    let window_size = 4;
    let mut sender = GoBackNSender::new(window_size);
    let mut receiver = GoBackNReceiver::new();

    // Send 4 packets to fill the window
    let mut sent_packets = Vec::new();
    for i in 0..4 {
        assert!(sender.can_send());
        let pkt = sender.send_chunk(vec![i as u8]).unwrap();
        sent_packets.push(pkt);
    }
    // Window is now full
    assert!(!sender.can_send());

    // Receiver accepts packet 0, 1, 2, 3
    let mut last_ack = None;
    for pkt in sent_packets {
        last_ack = receiver.handle_packet(&pkt);
    }

    // Cumulative ACK 3 confirms all 4 packets!
    let ack = last_ack.unwrap();
    assert_eq!(ack.seq_num, 3);
    assert!(sender.handle_ack(&ack));

    // Window has advanced completely!
    assert_eq!(sender.send_base(), 4);
    assert!(sender.can_send());
}

#[test]
fn test_gobackn_discards_out_of_order_packets() {
    let mut sender = GoBackNSender::new(4);
    let mut receiver = GoBackNReceiver::new();

    let pkt0 = sender.send_chunk(b"0".to_vec()).unwrap();
    let pkt1 = sender.send_chunk(b"1".to_vec()).unwrap();
    let pkt2 = sender.send_chunk(b"2".to_vec()).unwrap();

    // Receiver gets pkt0
    let ack0 = receiver.handle_packet(&pkt0).unwrap();
    sender.handle_ack(&ack0);

    // Simulate REORDERING: pkt2 arrives BEFORE pkt1!
    let dup_ack = receiver.handle_packet(&pkt2).unwrap();
    // GBN MUST discard out-of-order pkt2!
    assert_eq!(receiver.discarded_out_of_order, 1);
    assert_eq!(receiver.delivered_chunks().len(), 1);
    // Duplicate ACK for pkt0 sent back
    assert_eq!(dup_ack.seq_num, 0);

    // Sender times out on pkt1: retransmits both pkt1 AND pkt2 (the whole outstanding window)!
    let retransmits = sender.handle_timeout();
    assert_eq!(retransmits.len(), 2);
    assert_eq!(retransmits[0].seq_num, 1);
    assert_eq!(retransmits[1].seq_num, 2);
}

#[test]
fn test_selective_repeat_buffers_out_of_order_packets() {
    let window_size = 4;
    let mut sender = SelectiveRepeatSender::new(window_size);
    let mut receiver = SelectiveRepeatReceiver::new(window_size);

    let pkt0 = sender.send_chunk(b"chunk0".to_vec()).unwrap();
    let pkt1 = sender.send_chunk(b"chunk1".to_vec()).unwrap();
    let pkt2 = sender.send_chunk(b"chunk2".to_vec()).unwrap();

    // Receiver gets pkt0
    let ack0 = receiver.handle_packet(&pkt0).unwrap();
    sender.handle_ack(&ack0);
    assert_eq!(receiver.delivered_chunks().len(), 1);

    // REORDERING: pkt2 arrives BEFORE pkt1!
    // In Selective Repeat: Receiver BUFFERS pkt2 and sends individual ACK(2)!
    let ack2 = receiver.handle_packet(&pkt2).unwrap();
    assert_eq!(ack2.seq_num, 2);
    assert_eq!(receiver.buffered_count(), 1);
    // Chunk 2 is buffered, not yet delivered because waiting for missing chunk 1!
    assert_eq!(receiver.delivered_chunks().len(), 1);

    // Sender receives individual ACK(2)
    assert!(sender.handle_ack(&ack2));

    // Now missing pkt1 arrives!
    let ack1 = receiver.handle_packet(&pkt1).unwrap();
    assert_eq!(ack1.seq_num, 1);

    // In-order consecutive drain: BOTH chunk 1 AND chunk 2 are now delivered!
    assert_eq!(receiver.delivered_chunks().len(), 3);
    assert_eq!(receiver.buffered_count(), 0);
    assert_eq!(receiver.delivered_chunks()[0], b"chunk0");
    assert_eq!(receiver.delivered_chunks()[1], b"chunk1");
    assert_eq!(receiver.delivered_chunks()[2], b"chunk2");

    // Primary Claim Verified: Selective Repeat required ZERO retransmissions under reordering!
    assert_eq!(sender.retransmissions, 0);
}
