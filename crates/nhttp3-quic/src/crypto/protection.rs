use crate::packet::PacketError;

/// Applies header protection to a packet (RFC 9001 §5.4).
///
/// The sample is taken from `pn_offset + 4`. Protection is applied to the
/// first byte and the packet number bytes using the HP key.
pub fn apply_header_protection(
    hp_key: &dyn rustls::quic::HeaderProtectionKey,
    packet: &mut [u8],
    pn_offset: usize,
) -> Result<(), PacketError> {
    let sample_offset = pn_offset + 4;
    let sample_len = hp_key.sample_len();

    if packet.len() < sample_offset + sample_len {
        return Err(PacketError::Invalid(
            "packet too short for header protection sample".into(),
        ));
    }

    let sample = packet[sample_offset..sample_offset + sample_len].to_vec();

    // Determine PN length from first byte (before protection, low 2 bits + 1)
    let pn_len = (packet[0] & 0x03) as usize + 1;

    let (first, rest) = packet.split_at_mut(1);
    let pn_bytes = &mut rest[pn_offset - 1..pn_offset - 1 + pn_len];

    hp_key
        .encrypt_in_place(&sample, &mut first[0], pn_bytes)
        .map_err(|e| PacketError::Invalid(format!("header protection failed: {e}")))?;

    Ok(())
}

/// Removes header protection from a packet.
pub fn remove_header_protection(
    hp_key: &dyn rustls::quic::HeaderProtectionKey,
    packet: &mut [u8],
    pn_offset: usize,
) -> Result<(), PacketError> {
    let sample_offset = pn_offset + 4;
    let sample_len = hp_key.sample_len();

    if packet.len() < sample_offset + sample_len {
        return Err(PacketError::Invalid(
            "packet too short for header protection sample".into(),
        ));
    }

    let sample = packet[sample_offset..sample_offset + sample_len].to_vec();

    // For removal, we first need to decrypt the first byte to know PN length
    // We temporarily decrypt with max PN length (4), then fix up
    let pn_len = 4.min(packet.len() - pn_offset);
    let (first, rest) = packet.split_at_mut(1);
    let pn_bytes = &mut rest[pn_offset - 1..pn_offset - 1 + pn_len];

    hp_key
        .decrypt_in_place(&sample, &mut first[0], pn_bytes)
        .map_err(|e| PacketError::Invalid(format!("header unprotection failed: {e}")))?;

    Ok(())
}
