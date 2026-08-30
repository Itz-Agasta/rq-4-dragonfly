//! CAN frame layout, transfer splitting, and reassembly.
//!
//! A transfer is one serialised message. If it fits in seven bytes it becomes one
//! frame; otherwise a CRC is prepended to the payload and the result is cut into
//! seven-byte pieces, every frame carrying the same CAN ID and differing only in
//! its tail byte.
//!
//! No clock and no I/O. Reassembly needs a monotonic time to expire a stalled
//! transfer, so the caller passes one in. That keeps the whole crate testable
//! without a runtime and lets replay drive it from recorded timestamps rather
//! than from wall time.
//!
//! <https://dronecan.github.io/Specification/4.1_CAN_bus_transport_layer/>

use std::collections::HashMap;
use std::time::Duration;

use crate::crc::TransferCrc;

/// Payload bytes available per frame once the tail byte is deducted.
pub const BYTES_PER_FRAME: usize = 7;

/// A transfer whose last matching frame is older than this is abandoned.
pub const TRANSFER_ID_TIMEOUT: Duration = Duration::from_secs(2);

/// Largest transfer that will be reassembled, bytes.
///
/// Comfortably above the biggest standard data type, which is under 400 bytes.
/// A bound is needed at all because a node that keeps emitting frames of one
/// transfer and never ends it would otherwise grow the buffer without limit, and
/// a health monitor sharing a bus with a misbehaving device has to survive it.
pub const MAX_TRANSFER_BYTES: usize = 512;

/// A CAN 2.0B data frame with a 29-bit identifier.
///
/// Defined here rather than taken from a SocketCAN type so the codec can be
/// tested, replayed and published without a Linux dependency.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Frame {
    id: u32,
    data: [u8; 8],
    len: u8,
}

impl Frame {
    /// Build a frame, or `None` if the payload exceeds eight bytes.
    #[must_use]
    pub fn new(id: u32, data: &[u8]) -> Option<Self> {
        if data.len() > 8 {
            return None;
        }
        let mut buf = [0u8; 8];
        buf[..data.len()].copy_from_slice(data);
        Some(Self {
            id: id & 0x1FFF_FFFF,
            data: buf,
            len: data.len() as u8,
        })
    }

    /// The 29-bit identifier.
    #[must_use]
    pub fn id(&self) -> u32 {
        self.id
    }

    /// The data field.
    #[must_use]
    pub fn data(&self) -> &[u8] {
        &self.data[..self.len as usize]
    }
}

/// The fields a message frame's CAN identifier carries.
///
/// From the most significant bit of the 29: priority 5, data type ID 16,
/// service-not-message 1 (always zero for a message), source node ID 7.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MessageId {
    /// 0 is the most urgent, 31 the least.
    pub priority: u8,
    /// Data type ID of the message being carried.
    pub data_type_id: u16,
    /// Node that sent it, 1 to 127.
    pub source_node_id: u8,
}

impl MessageId {
    /// Pack into a 29-bit identifier.
    #[must_use]
    pub fn to_raw(self) -> u32 {
        (u32::from(self.priority & 0x1F) << 24)
            | (u32::from(self.data_type_id) << 8)
            | u32::from(self.source_node_id & 0x7F)
    }

    /// Unpack a 29-bit identifier, or `None` if it describes a service transfer
    /// or an anonymous message, neither of which this crate handles.
    #[must_use]
    pub fn from_raw(raw: u32) -> Option<Self> {
        let raw = raw & 0x1FFF_FFFF;
        if raw & 0x80 != 0 {
            return None;
        }
        let source_node_id = (raw & 0x7F) as u8;
        if source_node_id == 0 {
            return None;
        }
        Some(Self {
            priority: ((raw >> 24) & 0x1F) as u8,
            data_type_id: ((raw >> 8) & 0xFFFF) as u16,
            source_node_id,
        })
    }
}

/// The last byte of every frame's data field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Tail {
    start: bool,
    end: bool,
    toggle: bool,
    transfer_id: u8,
}

impl Tail {
    const fn to_byte(self) -> u8 {
        ((self.start as u8) << 7)
            | ((self.end as u8) << 6)
            | ((self.toggle as u8) << 5)
            | (self.transfer_id & 0x1F)
    }

    const fn from_byte(b: u8) -> Self {
        Self {
            start: b & 0x80 != 0,
            end: b & 0x40 != 0,
            toggle: b & 0x20 != 0,
            transfer_id: b & 0x1F,
        }
    }
}

/// Split a serialised message into the frames that carry it.
///
/// `signature` seeds the transfer CRC and is only consulted for a multi-frame
/// transfer; a single-frame transfer carries no CRC because the CAN controller's
/// own check covers it.
#[must_use]
pub fn frames_for(id: MessageId, signature: u64, payload: &[u8], transfer_id: u8) -> Vec<Frame> {
    let raw = id.to_raw();
    let mut out = Vec::new();

    if payload.len() <= BYTES_PER_FRAME {
        let mut data = payload.to_vec();
        data.push(
            Tail {
                start: true,
                end: true,
                toggle: false,
                transfer_id,
            }
            .to_byte(),
        );
        out.extend(Frame::new(raw, &data));
        return out;
    }

    let crc = TransferCrc::seeded(signature).extend(payload).get();
    let mut body = Vec::with_capacity(payload.len() + 2);
    body.extend_from_slice(&crc.to_le_bytes());
    body.extend_from_slice(payload);

    let chunks = body.len().div_ceil(BYTES_PER_FRAME);
    for (index, chunk) in body.chunks(BYTES_PER_FRAME).enumerate() {
        let mut data = chunk.to_vec();
        data.push(
            Tail {
                start: index == 0,
                end: index + 1 == chunks,
                toggle: !index.is_multiple_of(2),
                transfer_id,
            }
            .to_byte(),
        );
        out.extend(Frame::new(raw, &data));
    }
    out
}

/// A complete transfer, reassembled and stripped of its transport framing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Transfer {
    /// Identifier fields the frames carried.
    pub id: MessageId,
    /// Transfer ID, 0 to 31.
    pub transfer_id: u8,
    /// The serialised message.
    pub payload: Vec<u8>,
    /// Checksum found on the wire, absent for a single-frame transfer.
    pub crc: Option<u16>,
}

impl Transfer {
    /// Whether the payload matches the checksum for a data type of this signature.
    ///
    /// A single-frame transfer has no checksum to disagree with, so it passes.
    #[must_use]
    pub fn crc_ok(&self, signature: u64) -> bool {
        match self.crc {
            None => true,
            Some(crc) => TransferCrc::seeded(signature).extend(&self.payload).get() == crc,
        }
    }
}

/// Per-publisher transfer ID counters.
///
/// One rolling 5-bit counter per transfer descriptor. The descriptor includes
/// the source node ID, so a process publishing as more than one node needs a
/// counter per node: sharing one makes a receiver see gaps and drop transfers.
#[derive(Debug, Default)]
pub struct TransferIdMap(HashMap<(u8, u16), u8>);

impl TransferIdMap {
    /// An empty map. Every descriptor starts at zero.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The next transfer ID for this descriptor, incrementing the counter.
    pub fn next(&mut self, source_node_id: u8, data_type_id: u16) -> u8 {
        let slot = self.0.entry((source_node_id, data_type_id)).or_insert(0);
        let value = *slot;
        *slot = (value + 1) & 0x1F;
        value
    }
}

#[derive(Debug)]
struct Partial {
    payload: Vec<u8>,
    transfer_id: u8,
    toggle: bool,
    started: Duration,
}

/// Rebuilds transfers from the frames arriving on one interface.
///
/// Follows the specification's non-redundant reception pseudocode: a frame whose
/// toggle or transfer ID does not match the state is dropped rather than
/// appended, so a lost middle frame abandons its transfer instead of splicing
/// two halves of different ones together.
#[derive(Debug, Default)]
pub struct Reassembler {
    states: HashMap<u32, Partial>,
}

impl Reassembler {
    /// A reassembler with no transfers in flight.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed one frame, yielding a transfer when its last frame arrives.
    ///
    /// `now` is any monotonic time; it is only compared against itself, to expire
    /// a transfer whose remaining frames never arrived.
    pub fn push(&mut self, frame: &Frame, now: Duration) -> Option<Transfer> {
        let id = MessageId::from_raw(frame.id())?;
        let (tail, body) = frame.data().split_last()?;
        let tail = Tail::from_byte(*tail);

        let key = frame.id();
        let timed_out = self
            .states
            .get(&key)
            .is_none_or(|p| now.saturating_sub(p.started) > TRANSFER_ID_TIMEOUT);

        if tail.start {
            self.states.insert(
                key,
                Partial {
                    payload: Vec::new(),
                    transfer_id: tail.transfer_id,
                    toggle: false,
                    started: now,
                },
            );
        } else if timed_out {
            // The start of this transfer was missed; there is nothing to append to.
            self.states.remove(&key);
            return None;
        }

        let state = self.states.get_mut(&key)?;
        if tail.toggle != state.toggle || tail.transfer_id != state.transfer_id {
            self.states.remove(&key);
            return None;
        }
        if state.payload.len() + body.len() > MAX_TRANSFER_BYTES {
            self.states.remove(&key);
            return None;
        }
        state.toggle = !state.toggle;
        state.payload.extend_from_slice(body);

        if !tail.end {
            return None;
        }

        let Partial {
            mut payload,
            transfer_id,
            ..
        } = self.states.remove(&key)?;

        // Single-frame transfers carry no CRC; multi-frame ones lead with it.
        let crc = if tail.start {
            None
        } else if payload.len() >= 2 {
            let value = u16::from_le_bytes([payload[0], payload[1]]);
            payload.drain(..2);
            Some(value)
        } else {
            return None;
        };

        Some(Transfer {
            id,
            transfer_id,
            payload,
            crc,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: MessageId = MessageId {
        priority: 16,
        data_type_id: 1120,
        source_node_id: 42,
    };

    /// The identifier a reference-encoded `reciprocating.Status` from node 42 at
    /// priority 16 actually carries.
    #[test]
    fn message_id_matches_a_reference_frame() {
        assert_eq!(ID.to_raw(), 0x1004_602A);
        assert_eq!(MessageId::from_raw(0x1004_602A), Some(ID));
    }

    #[test]
    fn service_and_anonymous_identifiers_are_rejected() {
        assert_eq!(MessageId::from_raw(ID.to_raw() | 0x80), None);
        assert_eq!(MessageId::from_raw(ID.to_raw() & !0x7F), None);
    }

    #[test]
    fn a_short_payload_becomes_one_frame_with_no_crc() {
        let frames = frames_for(ID, 0, &[1, 2, 3], 7);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].data(), &[1, 2, 3, 0b1100_0111]);

        let mut r = Reassembler::new();
        let t = r.push(&frames[0], Duration::ZERO).expect("one frame");
        assert_eq!(t.payload, vec![1, 2, 3]);
        assert_eq!(t.crc, None);
        assert!(t.crc_ok(0));
    }

    #[test]
    fn a_long_payload_round_trips_through_reassembly() {
        let payload: Vec<u8> = (0..75).collect();
        let frames = frames_for(ID, 0xDEAD_BEEF_1234_5678, &payload, 7);
        assert_eq!(frames.len(), 11);
        assert_eq!(frames[0].data()[7], 0b1000_0111);
        assert_eq!(frames[1].data()[7], 0b0010_0111);
        assert_eq!(frames[10].data()[7], 0b0100_0111);

        let mut r = Reassembler::new();
        let mut got = None;
        for f in &frames {
            got = r.push(f, Duration::ZERO).or(got);
        }
        let t = got.expect("transfer completes");
        assert_eq!(t.payload, payload);
        assert!(t.crc_ok(0xDEAD_BEEF_1234_5678));
        assert!(!t.crc_ok(0));
    }

    /// The failure this guards against is the worst one available: splicing the
    /// head of one transfer onto the tail of the next and decoding the result.
    #[test]
    fn a_dropped_middle_frame_abandons_the_transfer() {
        let payload: Vec<u8> = (0..75).collect();
        let frames = frames_for(ID, 1, &payload, 7);
        let mut r = Reassembler::new();
        for (i, f) in frames.iter().enumerate() {
            if i == 5 {
                continue;
            }
            assert_eq!(r.push(f, Duration::ZERO), None, "frame {i}");
        }
    }

    #[test]
    fn a_repeated_frame_is_rejected_by_the_toggle_bit() {
        let payload: Vec<u8> = (0..75).collect();
        let frames = frames_for(ID, 1, &payload, 7);
        let mut r = Reassembler::new();
        assert_eq!(r.push(&frames[0], Duration::ZERO), None);
        assert_eq!(r.push(&frames[1], Duration::ZERO), None);
        assert_eq!(r.push(&frames[1], Duration::ZERO), None);
        for f in &frames[2..] {
            assert_eq!(r.push(f, Duration::ZERO), None);
        }
    }

    #[test]
    fn transfer_ids_roll_over_at_thirty_two() {
        let mut map = TransferIdMap::new();
        for expected in 0..32 {
            assert_eq!(map.next(42, 1120), expected);
        }
        assert_eq!(map.next(42, 1120), 0);
        assert_eq!(map.next(42, 1129), 0);
        // Same data type, different node: a separate counter.
        assert_eq!(map.next(43, 1120), 0);
    }

    /// A node that never ends a transfer must not be able to exhaust memory.
    #[test]
    fn an_endless_transfer_is_abandoned_rather_than_buffered() {
        let mut r = Reassembler::new();
        let raw = ID.to_raw();
        let mut toggle = false;
        let mut start = true;
        for _ in 0..200 {
            let tail = Tail {
                start,
                end: false,
                toggle,
                transfer_id: 7,
            };
            let mut data = vec![0u8; BYTES_PER_FRAME];
            data.push(tail.to_byte());
            let frame = Frame::new(raw, &data).expect("fits");
            assert_eq!(r.push(&frame, Duration::ZERO), None);
            toggle = !toggle;
            start = false;
        }
        assert!(r.states.is_empty(), "the partial transfer was not dropped");
    }
}
