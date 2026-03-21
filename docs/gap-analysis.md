# Gap Analysis: nhttp3 vs aioquic/quiche issues

## Critical Security Issues (MUST address)

1. **Unbounded ACK ranges DoS** (aioquic #549) — Malicious ACK frames with excessive ranges can crash or OOM. Our `Frame::parse` for ACK doesn't limit ack_range_count.

2. **Unbounded CRYPTO frames** (aioquic #501) — Unlimited CRYPTO frame accumulation per connection. Our ConnectionInner doesn't limit buffered handshake data.

3. **Unlimited path challenges** (aioquic #544) — Storing unlimited remote PATH_CHALLENGE data. Our PathValidator is single-instance but we should cap.

4. **Packet size validation** (aioquic #401, #325) — Initial packets MUST have UDP payload >= 1200 bytes. We don't enforce this.

5. **Forgeable retry tokens** (quiche #2334) — Retry tokens need encryption/authentication. We haven't implemented Retry at all yet.

6. **STOP_SENDING → RESET_STREAM** (aioquic #629) — Protocol violation when STOP_SENDING triggers RESET_STREAM with invalid final_size.

## Protocol Correctness Issues

7. **Initial packet DCID validation** (aioquic #627) — Client should reject handshake messages in Initial packets (should be in Handshake packets).

8. **Duplicate RETIRE_CONNECTION_ID** (quiche #1833) — Should not close connection on duplicate.

9. **Idle timeout calculation** (aioquic #466) — Should be min(local, remote) not just local.

10. **Premature Handshake key discard** (aioquic #622) — Keys discarded before all handshake data is acked.

11. **FIN swallowed when data fully ACKed** (aioquic #438) — Edge case in stream state machine.

12. **Large HEADERS frame** (quiche #2252) — Incorrect handling of HEADERS frames that span multiple QUIC packets.

13. **GREASE and push_promise stream close** (aioquic #565) — Trailing frames should close stream properly.

## Missing Features We Should Track

14. **WebTransport** (aioquic #630, quiche #2258) — RFC 9297 support
15. **Multipath QUIC** (aioquic #595) — Draft extension
16. **QLOG support** (quiche #2378, aioquic #513) — Standardized logging
17. **Session resumption / 0-RTT** (aioquic #492, quiche #1803)
18. **Address validation tokens** (quiche #2395)
19. **BBR congestion control** (quiche #1829)
20. **SO_REUSEPORT** (aioquic #480) — For multi-process servers
21. **Stateless reset** (aioquic #555) — Oracle attack prevention
22. **MoQ support** (quiche #2140) — Media over QUIC

## Performance Issues

23. **Zero-copy stream read** (quiche #1966) — Avoid copying on stream read
24. **Pacing** (quiche #1829) — BBR pacing rate
25. **LTO optimization** (quiche #1892) — Build optimization

## Our Current Gaps (Priority Order)

### P0 — Security (fix before any production use)
- [ ] Limit ACK range count in Frame::parse (cap at 256)
- [ ] Limit CRYPTO frame buffer size per connection
- [ ] Enforce 1200-byte minimum for Initial packets
- [ ] Add Retry token generation with AEAD encryption

### P1 — Protocol correctness
- [ ] Proper idle timeout (min of local/remote)
- [ ] STOP_SENDING → RESET_STREAM with correct final_size
- [ ] Duplicate RETIRE_CONNECTION_ID handling
- [ ] Initial packet DCID validation
- [ ] Stream FIN handling edge cases

### P2 — Missing features (roadmap)
- [ ] QLOG support
- [ ] Session resumption / 0-RTT (end-to-end)
- [ ] WebTransport (RFC 9297)
- [ ] BBR congestion control
- [ ] Stateless reset
- [ ] SO_REUSEPORT
