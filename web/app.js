// CS-30003: Reliable Data Transfer over UDP - iOS Protocol Inspector Engine
// Implements Stop-and-Wait, Go-Back-N, and Selective Repeat with RFC 1071 Checksum & RFC 6298 RTO

(function () {
  'use strict';

  // --- RFC 1071 One's Complement Checksum Implementation ---
  function computeRFC1071Checksum(headerBytes, payloadBytes) {
    let sum = 0;
    const combined = [...headerBytes, ...payloadBytes];
    for (let i = 0; i < combined.length; i += 2) {
      const b1 = combined[i];
      const b2 = (i + 1 < combined.length) ? combined[i + 1] : 0;
      const word = (b1 << 8) | b2;
      sum += word;
      while (sum > 0xFFFF) {
        sum = (sum & 0xFFFF) + (sum >> 16);
      }
    }
    return (~sum) & 0xFFFF;
  }

  // --- Simulation State ---
  const state = {
    protocol: 'gbn', // 'sw' | 'gbn' | 'sr'
    windowSize: 4,
    latencyMs: 400,
    lossRate: 0,
    ackLossRate: 0,
    simSpeed: 1.0,

    // Impairment One-Shots
    dropNextPkt: false,
    corruptNextPkt: false,
    dropNextAck: false,

    // Sender State
    sendBase: 0,
    nextSeqNum: 0,
    senderBuffer: [], // { seq, payload, state: 'usable'|'inflight'|'acked', timer: { elapsed, rto }, retrans: 0 }
    
    // Receiver State
    expectedSeqNum: 0,
    rcvBase: 0, // For SR
    rcvBuffer: {}, // seq -> payload (For SR)
    deliveredChunks: [],

    // Channel (In-Flight Packets)
    flyingPackets: [], // { id, type: 'DATA'|'ACK', seq, payload, progress: 0..1, dropped: false, corrupt: false, ... }

    // Metrics
    totalSent: 0,
    totalRetrans: 0,
    totalAcked: 0,
    srtt: 400,
    rttvar: 100,
    rto: 1000,

    // Engine Loop
    autoStream: false,
    streamTimer: null,
    lastFrameTime: performance.now(),
    pktIdCounter: 100,
  };

  // Sample payload sentences for realistic data transmission
  const sampleSentences = [
    "RELIABLE_UDP_CHUNK_ALPHA_1400",
    "SLIDING_WINDOW_PACKET_BETA_2800",
    "RFC1071_CHECKSUM_VALIDATED_4200",
    "GO_BACK_N_CUMULATIVE_ACK_5600",
    "SELECTIVE_REPEAT_BUFFER_7000",
    "FLOW_CONTROL_CONGESTION_AVOID",
    "HIGH_THROUGHPUT_DATAGRAM_STREAM",
    "PAYLOAD_INTEGRITY_VERIFIED_OK",
    "SOCKET_BUFFER_QUEUE_DRAINED_NOW",
    "END_OF_TRANSMISSION_FINAL_CHUNK"
  ];

  // --- DOM Elements ---
  const el = {
    statusClock: document.getElementById('statusClock'),
    dynamicIsland: document.getElementById('dynamicIsland'),
    islandPulse: document.getElementById('islandPulse'),
    islandProtocolLabel: document.getElementById('islandProtocolLabel'),
    islandStatusBadge: document.getElementById('islandStatusBadge'),
    islandInFlight: document.getElementById('islandInFlight'),
    islandRtt: document.getElementById('islandRtt'),
    islandLoss: document.getElementById('islandLoss'),

    tabSW: document.getElementById('tabSW'),
    tabGBN: document.getElementById('tabGBN'),
    tabSR: document.getElementById('tabSR'),
    segIndicator: document.getElementById('segIndicator'),

    btnSendChunk: document.getElementById('btnSendChunk'),
    btnStreamToggle: document.getElementById('btnStreamToggle'),
    streamPlayIcon: document.getElementById('streamPlayIcon'),
    streamText: document.getElementById('streamText'),
    btnStep: document.getElementById('btnStep'),
    btnReset: document.getElementById('btnReset'),

    btnDropNext: document.getElementById('btnDropNext'),
    btnCorruptNext: document.getElementById('btnCorruptNext'),
    btnDropAckNext: document.getElementById('btnDropAckNext'),

    lblSendBase: document.getElementById('lblSendBase'),
    lblNextSeq: document.getElementById('lblNextSeq'),
    lblWindowRatio: document.getElementById('lblWindowRatio'),
    senderBufferTrack: document.getElementById('senderBufferTrack'),
    activeTimersContainer: document.getElementById('activeTimersContainer'),
    lblRTO: document.getElementById('lblRTO'),

    channelStatusText: document.getElementById('channelStatusText'),
    laneData: document.getElementById('laneData'),
    laneAck: document.getElementById('laneAck'),
    flightCorridor: document.getElementById('flightCorridor'),
    hubActionText: document.getElementById('hubActionText'),

    lblExpectedSeq: document.getElementById('lblExpectedSeq'),
    lblRcvMode: document.getElementById('lblRcvMode'),
    receiverBufferTrack: document.getElementById('receiverBufferTrack'),
    lblDeliveredCount: document.getElementById('lblDeliveredCount'),
    deliveredStreamBox: document.getElementById('deliveredStreamBox'),

    sliderLatency: document.getElementById('sliderLatency'),
    valLatency: document.getElementById('valLatency'),
    sliderLoss: document.getElementById('sliderLoss'),
    valLoss: document.getElementById('valLoss'),
    sliderAckLoss: document.getElementById('sliderAckLoss'),
    valAckLoss: document.getElementById('valAckLoss'),
    sliderWindow: document.getElementById('sliderWindow'),
    valWindow: document.getElementById('valWindow'),
    rowWindowSize: document.getElementById('rowWindowSize'),

    ladderEvents: document.getElementById('ladderEvents'),
    ladderCanvasWrap: document.getElementById('ladderCanvasWrap'),
    btnClearChart: document.getElementById('btnClearChart'),

    metricTotalSent: document.getElementById('metricTotalSent'),
    metricRetrans: document.getElementById('metricRetrans'),
    metricOverhead: document.getElementById('metricOverhead'),
    metricSRTT: document.getElementById('metricSRTT'),
    metricGoodput: document.getElementById('metricGoodput'),

    // Inspector Modal
    packetModalBackdrop: document.getElementById('packetModalBackdrop'),
    packetInspectorSheet: document.getElementById('packetInspectorSheet'),
    btnCloseSheet: document.getElementById('btnCloseSheet'),
    sheetBadge: document.getElementById('sheetBadge'),
    sheetTitle: document.getElementById('sheetTitle'),
    sheetSeqHex: document.getElementById('sheetSeqHex'),
    sheetSeqDec: document.getElementById('sheetSeqDec'),
    sheetTypeHex: document.getElementById('sheetTypeHex'),
    sheetTypeDec: document.getElementById('sheetTypeDec'),
    sheetFlagsHex: document.getElementById('sheetFlagsHex'),
    sheetFlagsDec: document.getElementById('sheetFlagsDec'),
    sheetLenHex: document.getElementById('sheetLenHex'),
    sheetLenDec: document.getElementById('sheetLenDec'),
    sheetCsumHex: document.getElementById('sheetCsumHex'),
    sheetCsumDec: document.getElementById('sheetCsumDec'),
    sheetPayloadSize: document.getElementById('sheetPayloadSize'),
    sheetHexDump: document.getElementById('sheetHexDump'),
  };

  // --- Clock Updater ---
  function updateClock() {
    const now = new Date();
    let hours = now.getHours();
    let minutes = now.getMinutes();
    minutes = minutes < 10 ? '0' + minutes : minutes;
    el.statusClock.textContent = `${hours}:${minutes}`;
  }
  setInterval(updateClock, 1000);
  updateClock();

  // --- Dynamic Island Expansion ---
  el.dynamicIsland.addEventListener('click', () => {
    el.dynamicIsland.classList.toggle('expanded');
  });

  // --- Segmented Control Positioning ---
  function updateSegmentedIndicator() {
    const activeBtn = document.querySelector('.seg-btn.active');
    if (activeBtn && el.segIndicator) {
      el.segIndicator.style.width = `${activeBtn.offsetWidth}px`;
      el.segIndicator.style.transform = `translateX(${activeBtn.offsetLeft - 3}px)`;
    }
  }

  function setProtocol(proto) {
    state.protocol = proto;
    document.querySelectorAll('.seg-btn').forEach(b => b.classList.remove('active'));
    if (proto === 'sw') {
      el.tabSW.classList.add('active');
      state.windowSize = 1;
      el.rowWindowSize.style.opacity = '0.4';
      el.rowWindowSize.style.pointerEvents = 'none';
      el.lblRcvMode.textContent = 'Stop-and-Wait (W=1)';
    } else if (proto === 'gbn') {
      el.tabGBN.classList.add('active');
      state.windowSize = parseInt(el.sliderWindow.value, 10);
      el.rowWindowSize.style.opacity = '1';
      el.rowWindowSize.style.pointerEvents = 'auto';
      el.lblRcvMode.textContent = 'Cumulative In-Order (GBN)';
    } else if (proto === 'sr') {
      el.tabSR.classList.add('active');
      state.windowSize = parseInt(el.sliderWindow.value, 10);
      el.rowWindowSize.style.opacity = '1';
      el.rowWindowSize.style.pointerEvents = 'auto';
      el.lblRcvMode.textContent = 'Out-of-Order Selective Buffer (SR)';
    }
    updateSegmentedIndicator();
    resetSimulation();
  }

  el.tabSW.addEventListener('click', () => setProtocol('sw'));
  el.tabGBN.addEventListener('click', () => setProtocol('gbn'));
  el.tabSR.addEventListener('click', () => setProtocol('sr'));
  window.addEventListener('resize', updateSegmentedIndicator);
  setTimeout(updateSegmentedIndicator, 100);

  // --- Slider Listeners ---
  el.sliderLatency.addEventListener('input', (e) => {
    state.latencyMs = parseInt(e.target.value, 10);
    el.valLatency.textContent = `${state.latencyMs} ms`;
    recalcRTO(state.latencyMs * 2);
  });

  el.sliderLoss.addEventListener('input', (e) => {
    state.lossRate = parseInt(e.target.value, 10);
    el.valLoss.textContent = `${state.lossRate}%`;
    el.islandLoss.textContent = `${state.lossRate}%`;
  });

  el.sliderAckLoss.addEventListener('input', (e) => {
    state.ackLossRate = parseInt(e.target.value, 10);
    el.valAckLoss.textContent = `${state.ackLossRate}%`;
  });

  el.sliderWindow.addEventListener('input', (e) => {
    if (state.protocol !== 'sw') {
      state.windowSize = parseInt(e.target.value, 10);
      el.valWindow.textContent = `${state.windowSize}`;
      renderBuffers();
      updateDynamicIsland();
    }
  });

  // Speed Pills
  document.querySelectorAll('.speed-pill').forEach(pill => {
    pill.addEventListener('click', () => {
      document.querySelectorAll('.speed-pill').forEach(p => p.classList.remove('active'));
      pill.classList.add('active');
      state.simSpeed = parseFloat(pill.dataset.speed);
    });
  });

  // Impairment Buttons
  el.btnDropNext.addEventListener('click', () => {
    state.dropNextPkt = !state.dropNextPkt;
    el.btnDropNext.classList.toggle('active', state.dropNextPkt);
  });
  el.btnCorruptNext.addEventListener('click', () => {
    state.corruptNextPkt = !state.corruptNextPkt;
    el.btnCorruptNext.classList.toggle('active', state.corruptNextPkt);
  });
  el.btnDropAckNext.addEventListener('click', () => {
    state.dropNextAck = !state.dropNextAck;
    el.btnDropAckNext.classList.toggle('active', state.dropNextAck);
  });

  // Clear Ladder Chart
  el.btnClearChart.addEventListener('click', () => {
    el.ladderEvents.innerHTML = '';
  });

  // --- Reset Simulation ---
  function resetSimulation() {
    state.sendBase = 0;
    state.nextSeqNum = 0;
    state.senderBuffer = [];
    state.expectedSeqNum = 0;
    state.rcvBase = 0;
    state.rcvBuffer = {};
    state.deliveredChunks = [];
    state.flyingPackets = [];
    state.totalSent = 0;
    state.totalRetrans = 0;
    state.totalAcked = 0;

    if (state.autoStream) {
      toggleAutoStream();
    }

    // Populate initial unbuffered payload slots
    for (let i = 0; i < 20; i++) {
      state.senderBuffer.push({
        seq: i,
        payload: sampleSentences[i % sampleSentences.length],
        state: 'usable',
        timer: null,
        retrans: 0
      });
    }

    // Clear DOM lanes & streams
    el.laneData.querySelectorAll('.packet-capsule').forEach(p => p.remove());
    el.laneAck.querySelectorAll('.packet-capsule').forEach(p => p.remove());
    el.deliveredStreamBox.innerHTML = '<span class="empty-stream-msg">Awaiting transmission chunks...</span>';
    el.ladderEvents.innerHTML = '';

    renderBuffers();
    renderTimers();
    updateMetrics();
    updateDynamicIsland();
  }

  // --- RFC 6298 RTO Estimation ---
  function recalcRTO(measuredRTT) {
    const alpha = 0.125;
    const beta = 0.25;
    state.rttvar = (1 - beta) * state.rttvar + beta * Math.abs(state.srtt - measuredRTT);
    state.srtt = (1 - alpha) * state.srtt + alpha * measuredRTT;
    state.rto = Math.max(800, Math.round(state.srtt + 4 * state.rttvar));
    el.lblRTO.textContent = `RTO: ${state.rto}ms (RFC 6298)`;
    el.islandRtt.textContent = `${Math.round(state.srtt)}ms`;
  }

  // --- Send Chunk Action ---
  function canSend() {
    return (state.nextSeqNum - state.sendBase) < state.windowSize;
  }

  function sendChunk() {
    if (!canSend()) {
      flashHubNotice("Window Full (Flow Throttled)");
      return;
    }

    const slot = state.senderBuffer.find(s => s.seq === state.nextSeqNum);
    if (!slot) return;

    slot.state = 'inflight';
    const seq = state.nextSeqNum;
    state.nextSeqNum++;
    state.totalSent++;

    // Timer handling:
    // SW: single timer per packet.
    // GBN: single timer running on sendBase.
    // SR: individual timer per packet.
    if (state.protocol === 'sw' || state.protocol === 'sr') {
      slot.timer = { elapsed: 0, rto: state.rto };
    } else if (state.protocol === 'gbn') {
      // In GBN, timer runs for sendBase
      const baseSlot = state.senderBuffer.find(s => s.seq === state.sendBase);
      if (baseSlot && !baseSlot.timer) {
        baseSlot.timer = { elapsed: 0, rto: state.rto };
      }
    }

    // Launch packet on wire
    launchPacketOnWire('DATA', seq, slot.payload, slot.retrans > 0);

    renderBuffers();
    renderTimers();
    updateMetrics();
    updateDynamicIsland();
  }

  // --- Launch Packet on Animated Wire ---
  function launchPacketOnWire(type, seq, payload, isRetrans = false) {
    const pktId = state.pktIdCounter++;
    
    // Determine drops and corruptions
    let willDrop = false;
    let willCorrupt = false;

    if (type === 'DATA') {
      if (state.dropNextPkt) {
        willDrop = true;
        state.dropNextPkt = false;
        el.btnDropNext.classList.remove('active');
      } else if (Math.random() * 100 < state.lossRate) {
        willDrop = true;
      }

      if (state.corruptNextPkt) {
        willCorrupt = true;
        state.corruptNextPkt = false;
        el.btnCorruptNext.classList.remove('active');
      }
    } else { // ACK
      if (state.dropNextAck) {
        willDrop = true;
        state.dropNextAck = false;
        el.btnDropAckNext.classList.remove('active');
      } else if (Math.random() * 100 < state.ackLossRate) {
        willDrop = true;
      }
    }

    // Build raw packet byte structure (RFC 1071 10-byte header)
    const header = [
      (seq >> 24) & 0xFF, (seq >> 16) & 0xFF, (seq >> 8) & 0xFF, seq & 0xFF, // seq_num (4B)
      type === 'DATA' ? 0x01 : 0x02, // pkt_type (1B)
      isRetrans ? 0x01 : 0x00, // flags (1B)
      (payload.length >> 8) & 0xFF, payload.length & 0xFF, // len (2B)
      0x00, 0x00 // checksum placeholder
    ];
    const payloadBytes = Array.from(payload).map(c => c.charCodeAt(0));
    let checksum = computeRFC1071Checksum(header, payloadBytes);
    if (willCorrupt) {
      checksum = (checksum ^ 0xBEEF) & 0xFFFF; // Corrupt CRC
    }
    header[8] = (checksum >> 8) & 0xFF;
    header[9] = checksum & 0xFF;

    const pktObj = {
      id: pktId,
      type: type,
      seq: seq,
      payload: payload,
      isRetrans: isRetrans,
      willDrop: willDrop,
      willCorrupt: willCorrupt,
      checksum: checksum,
      header: header,
      progress: 0,
      lane: type === 'DATA' ? 'down' : 'up',
      startTime: performance.now(),
      element: null
    };

    // Create DOM Capsule
    const capsule = document.createElement('div');
    capsule.className = `packet-capsule pkt-${type.toLowerCase()} ${willCorrupt ? 'corrupted' : ''}`;
    capsule.id = `pkt-${pktId}`;
    capsule.innerHTML = `
      <span class="capsule-badge">${type}</span>
      <span>#${seq}</span>
      ${willCorrupt ? '<span style="font-size:9px;">[BAD CRC]</span>' : ''}
    `;

    capsule.addEventListener('click', (e) => {
      e.stopPropagation();
      openPacketInspector(pktObj);
    });

    if (type === 'DATA') {
      el.laneData.appendChild(capsule);
    } else {
      el.laneAck.appendChild(capsule);
    }

    pktObj.element = capsule;
    state.flyingPackets.push(pktObj);

    // Record on Ladder Sequence Diagram
    addLadderEvent(pktObj);
  }

  // --- Animation & Physics Engine Loop ---
  function tick(timestamp) {
    const dt = (timestamp - state.lastFrameTime) * state.simSpeed;
    state.lastFrameTime = timestamp;

    // 1. Update In-Flight Packets
    const travelTime = state.latencyMs; // One-way propagation
    const finishedIndices = [];

    for (let i = 0; i < state.flyingPackets.length; i++) {
      const pkt = state.flyingPackets[i];
      pkt.progress += dt / travelTime;

      // Check drop at halfway point (center hub)
      if (pkt.willDrop && pkt.progress >= 0.5) {
        triggerPacketDrop(pkt);
        finishedIndices.push(i);
        continue;
      }

      if (pkt.progress >= 1.0) {
        pkt.progress = 1.0;
        finishedIndices.push(i);
        onPacketArrived(pkt);
        continue;
      }

      // Update position on screen
      if (pkt.element) {
        const laneWidth = el.flightCorridor.offsetWidth - 140;
        let xPos;
        if (pkt.lane === 'down') {
          xPos = 20 + pkt.progress * laneWidth;
        } else {
          xPos = (el.flightCorridor.offsetWidth - 120) - pkt.progress * laneWidth;
        }
        pkt.element.style.transform = `translateX(${xPos}px)`;
      }
    }

    // Remove finished packets
    for (let i = finishedIndices.length - 1; i >= 0; i--) {
      const idx = finishedIndices[i];
      const p = state.flyingPackets[idx];
      if (p.element && p.element.parentNode && !p.willDrop) {
        p.element.remove();
      }
      state.flyingPackets.splice(idx, 1);
    }

    // 2. Update Sender Retransmission Timers
    updateSenderTimers(dt);

    // 3. Update HUD & Corridor text
    const inFlightCount = state.flyingPackets.length;
    el.islandInFlight.textContent = `${inFlightCount} pkts`;
    el.channelStatusText.textContent = inFlightCount > 0 
      ? `Channel Active • ${inFlightCount} packet(s) on wire` 
      : `Channel Clear • 0 pkts in transit`;

    requestAnimationFrame(tick);
  }
  requestAnimationFrame(tick);

  // --- Packet Arrival Handler ---
  function onPacketArrived(pkt) {
    if (pkt.type === 'DATA') {
      handleDataArrivalAtReceiver(pkt);
    } else {
      handleAckArrivalAtSender(pkt);
    }
  }

  // --- Packet Drop Handler (Shatter FX) ---
  function triggerPacketDrop(pkt) {
    if (!pkt.element) return;
    pkt.element.classList.add('dropped-disintegrate');
    flashHubNotice(`DROPPED ${pkt.type} #${pkt.seq}`);
    addLadderDropMarker(pkt);

    setTimeout(() => {
      if (pkt.element && pkt.element.parentNode) {
        pkt.element.remove();
      }
    }, 500);
  }

  function flashHubNotice(msg) {
    el.hubActionText.textContent = msg;
    el.hubActionText.style.color = '#ff3b30';
    setTimeout(() => {
      el.hubActionText.textContent = "UDP PIPE";
      el.hubActionText.style.color = "var(--ios-text-secondary)";
    }, 1200);
  }

  // --- Receiver Logic: RFC 1071 Check & State Machines ---
  function handleDataArrivalAtReceiver(pkt) {
    // 1. Verify RFC 1071 Checksum
    const calcCsum = computeRFC1071Checksum(pkt.header.slice(0, 8), Array.from(pkt.payload).map(c => c.charCodeAt(0)));
    const wireCsum = (pkt.header[8] << 8) | pkt.header[9];
    const isCorrupt = (calcCsum !== wireCsum);

    if (isCorrupt) {
      flashHubNotice(`Corrupt Checksum Seq #${pkt.seq} (Discarded)`);
      return;
    }

    // 2. Protocol Specific Handling
    if (state.protocol === 'sw' || state.protocol === 'gbn') {
      // In-Order Cumulative Acceptance
      if (pkt.seq === state.expectedSeqNum) {
        deliverToApplication(pkt.seq, pkt.payload);
        state.expectedSeqNum++;
        el.lblExpectedSeq.textContent = state.expectedSeqNum;

        // Cumulative ACK for this received packet
        launchPacketOnWire('ACK', pkt.seq, `ACK_${pkt.seq}`);
      } else {
        // Out of order: discard and re-send cumulative ACK for highest consecutive
        if (state.expectedSeqNum > 0) {
          launchPacketOnWire('ACK', state.expectedSeqNum - 1, `CUMULATIVE_ACK_${state.expectedSeqNum - 1}`);
        }
      }
    } else if (state.protocol === 'sr') {
      // Selective Repeat: Buffer within receive window [rcvBase, rcvBase + W - 1]
      const win = state.windowSize;
      if (pkt.seq >= state.rcvBase && pkt.seq < state.rcvBase + win) {
        state.rcvBuffer[pkt.seq] = pkt.payload;
        // Send individual ACK immediately
        launchPacketOnWire('ACK', pkt.seq, `SR_ACK_${pkt.seq}`);

        // If it equals rcvBase, deliver contiguous buffered packets
        while (state.rcvBuffer[state.rcvBase]) {
          deliverToApplication(state.rcvBase, state.rcvBuffer[state.rcvBase]);
          delete state.rcvBuffer[state.rcvBase];
          state.rcvBase++;
        }
        el.lblExpectedSeq.textContent = state.rcvBase;
      } else if (pkt.seq < state.rcvBase) {
        // Packet already delivered, re-send ACK so sender can clear window
        launchPacketOnWire('ACK', pkt.seq, `SR_ACK_REPEAT_${pkt.seq}`);
      }
    }

    renderBuffers();
  }

  function deliverToApplication(seq, payload) {
    state.deliveredChunks.push({ seq, payload });
    el.lblDeliveredCount.textContent = `${state.deliveredChunks.length} chunks received`;

    if (state.deliveredChunks.length === 1) {
      el.deliveredStreamBox.innerHTML = '';
    }

    const chunkLine = document.createElement('div');
    chunkLine.innerHTML = `<span style="color:var(--ios-teal)">[DELIVERED #${seq}]</span> "${payload}"`;
    el.deliveredStreamBox.appendChild(chunkLine);
    el.deliveredStreamBox.scrollTop = el.deliveredStreamBox.scrollHeight;
  }

  // --- Sender Logic: ACK Handling & Window Advancement ---
  function handleAckArrivalAtSender(pkt) {
    state.totalAcked++;
    recalcRTO(state.latencyMs * 2);

    if (state.protocol === 'sw') {
      // Stop and Wait
      const slot = state.senderBuffer.find(s => s.seq === pkt.seq);
      if (slot && slot.state === 'inflight') {
        slot.state = 'acked';
        slot.timer = null;
        state.sendBase = pkt.seq + 1;
      }
    } else if (state.protocol === 'gbn') {
      // Go-Back-N Cumulative ACK: ACK K confirms all packets <= K
      if (pkt.seq >= state.sendBase) {
        for (let s = state.sendBase; s <= pkt.seq; s++) {
          const slot = state.senderBuffer.find(b => b.seq === s);
          if (slot) {
            slot.state = 'acked';
            slot.timer = null;
          }
        }
        state.sendBase = pkt.seq + 1;

        // Restart timer for new sendBase if outstanding packets exist
        if (state.sendBase < state.nextSeqNum) {
          const nextBaseSlot = state.senderBuffer.find(b => b.seq === state.sendBase);
          if (nextBaseSlot) {
            nextBaseSlot.timer = { elapsed: 0, rto: state.rto };
          }
        }
      }
    } else if (state.protocol === 'sr') {
      // Selective Repeat: Mark specific packet as acked
      const slot = state.senderBuffer.find(s => s.seq === pkt.seq);
      if (slot) {
        slot.state = 'acked';
        slot.timer = null;
      }

      // Slide window past all consecutive acked packets
      while (true) {
        const baseSlot = state.senderBuffer.find(s => s.seq === state.sendBase);
        if (baseSlot && baseSlot.state === 'acked') {
          state.sendBase++;
        } else {
          break;
        }
      }
    }

    el.lblSendBase.textContent = state.sendBase;
    renderBuffers();
    renderTimers();
    updateMetrics();
    updateDynamicIsland();
  }

  // --- Retransmission Timer Engine ---
  function updateSenderTimers(dt) {
    if (state.protocol === 'sw' || state.protocol === 'sr') {
      // Per-packet timers
      for (const slot of state.senderBuffer) {
        if (slot.timer && slot.state === 'inflight') {
          slot.timer.elapsed += dt;
          if (slot.timer.elapsed >= slot.timer.rto) {
            triggerTimeoutRetransmission(slot);
          }
        }
      }
    } else if (state.protocol === 'gbn') {
      // Single timer for sendBase
      const baseSlot = state.senderBuffer.find(s => s.seq === state.sendBase);
      if (baseSlot && baseSlot.timer && baseSlot.state === 'inflight') {
        baseSlot.timer.elapsed += dt;
        if (baseSlot.timer.elapsed >= baseSlot.timer.rto) {
          triggerGbnTimeoutRetransmission();
        }
      }
    }

    renderTimers();
  }

  function triggerTimeoutRetransmission(slot) {
    slot.timer.elapsed = 0;
    slot.retrans++;
    state.totalRetrans++;
    flashHubNotice(`TIMEOUT: Retransmit #${slot.seq}`);
    launchPacketOnWire('DATA', slot.seq, slot.payload, true);
    updateMetrics();
  }

  function triggerGbnTimeoutRetransmission() {
    const baseSlot = state.senderBuffer.find(s => s.seq === state.sendBase);
    if (baseSlot && baseSlot.timer) {
      baseSlot.timer.elapsed = 0;
    }

    flashHubNotice(`GBN TIMEOUT: Retransmit Window [${state.sendBase}..${state.nextSeqNum - 1}]`);

    // Retransmit ALL unacknowledged packets currently in the window
    for (let s = state.sendBase; s < state.nextSeqNum; s++) {
      const slot = state.senderBuffer.find(b => b.seq === s);
      if (slot && slot.state === 'inflight') {
        slot.retrans++;
        state.totalRetrans++;
        launchPacketOnWire('DATA', slot.seq, slot.payload, true);
      }
    }
    updateMetrics();
  }

  // --- Buffer Renderers ---
  function renderBuffers() {
    el.lblSendBase.textContent = state.sendBase;
    el.lblNextSeq.textContent = state.nextSeqNum;
    el.lblWindowRatio.textContent = `Size: W=${state.windowSize}`;

    // Sender Track
    el.senderBufferTrack.innerHTML = '';
    const visibleSenderSlots = state.senderBuffer.slice(0, Math.max(16, state.nextSeqNum + 6));
    
    visibleSenderSlots.forEach(slot => {
      const slotDiv = document.createElement('div');
      slotDiv.className = `buffer-slot slot-${slot.state}`;
      
      // Highlight current window bounds
      if (slot.seq >= state.sendBase && slot.seq < state.sendBase + state.windowSize) {
        slotDiv.style.boxShadow = '0 0 0 1.5px rgba(255, 149, 0, 0.7)';
      }

      slotDiv.innerHTML = `
        <span class="slot-seq">#${slot.seq}</span>
        <span class="slot-state">${slot.state}</span>
      `;
      slotDiv.addEventListener('click', () => {
        openSlotInspector(slot);
      });
      el.senderBufferTrack.appendChild(slotDiv);
    });

    // Receiver Track
    el.receiverBufferTrack.innerHTML = '';
    const rcvStart = state.protocol === 'sr' ? state.rcvBase : state.expectedSeqNum;
    for (let i = 0; i < 16; i++) {
      const seq = rcvStart + i;
      const slotDiv = document.createElement('div');
      
      let rcvState = 'pending';
      if (state.protocol === 'sr' && state.rcvBuffer[seq]) {
        rcvState = 'buffered';
      } else if (seq < rcvStart) {
        rcvState = 'delivered';
      } else if (seq === rcvStart) {
        rcvState = 'expected';
      }

      slotDiv.className = `buffer-slot slot-${rcvState}`;
      slotDiv.innerHTML = `
        <span class="slot-seq">#${seq}</span>
        <span class="slot-state">${rcvState}</span>
      `;
      el.receiverBufferTrack.appendChild(slotDiv);
    }
  }

  // --- Render Timers ---
  function renderTimers() {
    el.activeTimersContainer.innerHTML = '';
    let activeFound = false;

    if (state.protocol === 'gbn') {
      const baseSlot = state.senderBuffer.find(s => s.seq === state.sendBase);
      if (baseSlot && baseSlot.timer && baseSlot.state === 'inflight') {
        activeFound = true;
        renderTimerRow(`GBN [${state.sendBase}]`, baseSlot.timer.elapsed, baseSlot.timer.rto);
      }
    } else {
      for (const slot of state.senderBuffer) {
        if (slot.timer && slot.state === 'inflight') {
          activeFound = true;
          renderTimerRow(`Pkt #${slot.seq}`, slot.timer.elapsed, slot.timer.rto);
        }
      }
    }

    if (!activeFound) {
      el.activeTimersContainer.innerHTML = '<div class="no-timers-hint">No active packet timers</div>';
    }
  }

  function renderTimerRow(label, elapsed, rto) {
    const pct = Math.min(100, (elapsed / rto) * 100);
    const isDanger = pct > 75;
    const row = document.createElement('div');
    row.className = 'timer-row';
    row.innerHTML = `
      <span class="timer-title">${label}</span>
      <div class="timer-progress-wrap">
        <div class="timer-progress-fill ${isDanger ? 'danger' : ''}" style="width: ${pct}%"></div>
      </div>
      <span style="font-size:10px; color:${isDanger ? 'var(--ios-red)' : 'var(--ios-text-secondary)'}">${Math.round(elapsed)}ms</span>
    `;
    el.activeTimersContainer.appendChild(row);
  }

  // --- Dynamic Island Updates ---
  function updateDynamicIsland() {
    el.islandProtocolLabel.textContent = `${state.protocol.toUpperCase()} [Window: ${state.nextSeqNum - state.sendBase}/${state.windowSize}]`;
    const inFlight = state.flyingPackets.length;
    if (inFlight > 0) {
      el.islandPulse.classList.add('active-flow');
      el.islandStatusBadge.textContent = 'STREAMING';
      el.islandStatusBadge.style.color = 'var(--ios-blue)';
      el.islandStatusBadge.style.background = 'rgba(0, 122, 255, 0.18)';
    } else {
      el.islandPulse.classList.remove('active-flow');
      el.islandStatusBadge.textContent = 'READY';
      el.islandStatusBadge.style.color = 'var(--ios-green)';
      el.islandStatusBadge.style.background = 'rgba(52, 199, 89, 0.18)';
    }
  }

  // --- Metrics Updates ---
  function updateMetrics() {
    el.metricTotalSent.textContent = state.totalSent;
    el.metricRetrans.textContent = state.totalRetrans;
    
    const overhead = state.totalSent > 0 
      ? ((state.totalRetrans / state.totalSent) * 100).toFixed(1) 
      : '0.0';
    el.metricOverhead.textContent = `${overhead}%`;
    el.metricSRTT.textContent = `${Math.round(state.srtt)} ms`;

    const goodput = (state.totalSent + state.totalRetrans) > 0
      ? (Math.max(0, 100 - parseFloat(overhead))).toFixed(1)
      : '100.0';
    el.metricGoodput.textContent = `${goodput}%`;
  }

  // --- Ladder Sequence Diagram ---
  function addLadderEvent(pkt) {
    const eventRow = document.createElement('div');
    eventRow.className = 'ladder-event';
    eventRow.id = `ladder-${pkt.id}`;

    const isData = pkt.type === 'DATA';
    const color = isData ? 'var(--ios-blue)' : 'var(--ios-green)';
    const x1 = isData ? '5%' : '95%';
    const x2 = isData ? '95%' : '5%';

    eventRow.innerHTML = `
      <svg class="ladder-arrow-svg" viewBox="0 0 300 24">
        <line x1="${x1}" y1="4" x2="${x2}" y2="20" stroke="${color}" stroke-width="1.8" stroke-dasharray="${pkt.isRetrans ? '4,4' : 'none'}"></line>
        <circle cx="${x2}" cy="20" r="3" fill="${color}"></circle>
      </svg>
      <div class="ladder-event-label" style="color: ${color}">
        ${pkt.type} #${pkt.seq} ${pkt.isRetrans ? '(Retrans)' : ''}
      </div>
    `;

    el.ladderEvents.appendChild(eventRow);
    el.ladderCanvasWrap.scrollTop = el.ladderCanvasWrap.scrollHeight;

    // Limit ladder rows to 30 for performance
    if (el.ladderEvents.children.length > 30) {
      el.ladderEvents.removeChild(el.ladderEvents.children[0]);
    }
  }

  function addLadderDropMarker(pkt) {
    const item = document.getElementById(`ladder-${pkt.id}`);
    if (item) {
      const label = item.querySelector('.ladder-event-label');
      if (label) {
        label.style.borderColor = 'var(--ios-red)';
        label.style.color = 'var(--ios-red)';
        label.textContent += ' [DROPPED ✕]';
      }
    }
  }

  // --- Auto-Stream Toggle ---
  function toggleAutoStream() {
    state.autoStream = !state.autoStream;
    if (state.autoStream) {
      el.btnStreamToggle.classList.add('ios-button-primary');
      el.streamPlayIcon.textContent = '⏸';
      el.streamText.textContent = 'Streaming...';
      state.streamTimer = setInterval(() => {
        if (canSend()) {
          sendChunk();
        }
      }, 700 / state.simSpeed);
    } else {
      el.btnStreamToggle.classList.remove('ios-button-primary');
      el.streamPlayIcon.textContent = '▶';
      el.streamText.textContent = 'Auto-Stream';
      clearInterval(state.streamTimer);
    }
  }

  // --- Buttons Wiring ---
  el.btnSendChunk.addEventListener('click', sendChunk);
  el.btnStreamToggle.addEventListener('click', toggleAutoStream);
  el.btnStep.addEventListener('click', () => {
    if (canSend()) sendChunk();
  });
  el.btnReset.addEventListener('click', resetSimulation);

  // --- Packet Inspector Sheet ---
  function openPacketInspector(pkt) {
    el.sheetBadge.textContent = `${pkt.type} PACKET`;
    el.sheetBadge.style.color = pkt.type === 'DATA' ? 'var(--ios-blue)' : 'var(--ios-green)';
    el.sheetTitle.textContent = `${pkt.type} #${pkt.seq} Protocol Wire Frame`;

    // Sequence Num
    el.sheetSeqHex.textContent = '0x' + pkt.seq.toString(16).padStart(8, '0').toUpperCase();
    el.sheetSeqDec.textContent = pkt.seq;

    // Type
    el.sheetTypeHex.textContent = pkt.type === 'DATA' ? '0x01' : '0x02';
    el.sheetTypeDec.textContent = pkt.type;

    // Flags
    el.sheetFlagsHex.textContent = pkt.isRetrans ? '0x01' : '0x00';
    el.sheetFlagsDec.textContent = pkt.isRetrans ? 'Retransmitted' : 'Original';

    // Length
    el.sheetLenHex.textContent = '0x' + pkt.payload.length.toString(16).padStart(4, '0').toUpperCase();
    el.sheetLenDec.textContent = `${pkt.payload.length} bytes`;

    // Checksum
    el.sheetCsumHex.textContent = '0x' + pkt.checksum.toString(16).padStart(4, '0').toUpperCase();
    if (pkt.willCorrupt) {
      el.sheetCsumDec.textContent = 'CORRUPT (MISMATCH) ✕';
      el.sheetCsumDec.className = 'f-dec';
      el.sheetCsumDec.style.color = 'var(--ios-red)';
    } else {
      el.sheetCsumDec.textContent = 'VALID RFC 1071 ✓';
      el.sheetCsumDec.className = 'f-dec green';
    }

    // Payload Dump
    el.sheetPayloadSize.textContent = `${pkt.payload.length} bytes`;
    let hexStr = '';
    let asciiStr = '';
    for (let i = 0; i < pkt.payload.length; i++) {
      const code = pkt.payload.charCodeAt(i);
      hexStr += code.toString(16).padStart(2, '0').toUpperCase() + ' ';
      asciiStr += (code >= 32 && code <= 126) ? pkt.payload[i] : '.';
      if ((i + 1) % 8 === 0) {
        hexStr += '  ';
      }
    }
    el.sheetHexDump.innerHTML = `
      <div style="color:var(--ios-text-tertiary); font-size:10px; margin-bottom:4px;">OFFSET   HEX BYTES                                       ASCII</div>
      <div>0000     ${hexStr.padEnd(48, ' ')} |${asciiStr}|</div>
    `;

    el.packetModalBackdrop.classList.add('open');
  }

  function openSlotInspector(slot) {
    const dummyPkt = {
      type: 'DATA',
      seq: slot.seq,
      payload: slot.payload,
      isRetrans: slot.retrans > 0,
      willCorrupt: false,
      checksum: computeRFC1071Checksum([0,0,0,slot.seq, 1, slot.retrans > 0 ? 1 : 0, 0, slot.payload.length], Array.from(slot.payload).map(c=>c.charCodeAt(0))),
      header: [0,0,0,slot.seq, 1, slot.retrans > 0 ? 1 : 0, 0, slot.payload.length, 0, 0]
    };
    openPacketInspector(dummyPkt);
  }

  el.btnCloseSheet.addEventListener('click', () => {
    el.packetModalBackdrop.classList.remove('open');
  });

  el.packetModalBackdrop.addEventListener('click', (e) => {
    if (e.target === el.packetModalBackdrop) {
      el.packetModalBackdrop.classList.remove('open');
    }
  });

  // Initial Boot
  resetSimulation();

})();
