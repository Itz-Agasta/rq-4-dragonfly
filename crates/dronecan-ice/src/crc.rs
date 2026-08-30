//! The two checksums DroneCAN uses, and neither is the one people expect.
//!
//! CRC-16-CCITT-FALSE protects a multi-frame transfer's payload. CRC-64-WE
//! hashes a normalised data type definition into the signature that seeds it, so
//! two nodes disagreeing about a message's shape fail the payload check rather
//! than silently decoding each other's fields into the wrong slots.
//!
//! <https://dronecan.github.io/Specification/4.1_CAN_bus_transport_layer/>

/// CRC-16-CCITT-FALSE: init `0xFFFF`, poly `0x1021`, no reflection, no output xor.
///
/// Incremental because a transfer's checksum is seeded with the data type
/// signature before any payload is fed in.
#[derive(Clone, Copy, Debug)]
pub struct TransferCrc(u16);

impl TransferCrc {
    /// A checksum over nothing.
    #[must_use]
    pub const fn new() -> Self {
        Self(0xFFFF)
    }

    /// A checksum seeded with a data type signature, which is how every transfer
    /// starts. The eight signature bytes are fed least significant first.
    #[must_use]
    pub const fn seeded(signature: u64) -> Self {
        Self::new().extend(&signature.to_le_bytes())
    }

    /// Feed more bytes.
    #[must_use]
    pub const fn extend(self, bytes: &[u8]) -> Self {
        let mut crc = self.0;
        let mut i = 0;
        while i < bytes.len() {
            crc ^= (bytes[i] as u16) << 8;
            let mut bit = 0;
            while bit < 8 {
                crc = if crc & 0x8000 != 0 {
                    (crc << 1) ^ 0x1021
                } else {
                    crc << 1
                };
                bit += 1;
            }
            i += 1;
        }
        Self(crc)
    }

    /// The checksum value.
    #[must_use]
    pub const fn get(self) -> u16 {
        self.0
    }
}

impl Default for TransferCrc {
    fn default() -> Self {
        Self::new()
    }
}

/// CRC-64-WE: init and output xor all ones, poly `0x42F0E1EBA9EA3693`, no reflection.
///
/// `const` so a message's signature is computed from its normalised definition at
/// compile time. Signatures pasted in as hex literals cannot be checked against
/// the definition they are supposed to describe, and a wrong one is invisible
/// until an unrelated node rejects the transfer.
#[must_use]
pub const fn crc64_we(bytes: &[u8]) -> u64 {
    const POLY: u64 = 0x42F0_E1EB_A9EA_3693;
    let mut crc: u64 = u64::MAX;
    let mut i = 0;
    while i < bytes.len() {
        crc ^= (bytes[i] as u64) << 56;
        let mut bit = 0;
        while bit < 8 {
            crc = if crc & (1 << 63) != 0 {
                (crc << 1) ^ POLY
            } else {
                crc << 1
            };
            bit += 1;
        }
        i += 1;
    }
    crc ^ u64::MAX
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Both check values are published with the algorithm parameters, which makes
    /// them the cheapest possible guard against a transcription error in the loop.
    #[test]
    fn published_check_values() {
        assert_eq!(TransferCrc::new().extend(b"123456789").get(), 0x29B1);
        assert_eq!(crc64_we(b"123456789"), 0x62EC_59E3_F1A4_F00A);
    }

    #[test]
    fn feeding_in_pieces_matches_feeding_at_once() {
        let whole = TransferCrc::new().extend(b"123456789").get();
        let split = TransferCrc::new().extend(b"1234").extend(b"56789").get();
        assert_eq!(whole, split);
    }
}
