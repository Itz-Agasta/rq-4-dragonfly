//! The message set, one module per DSDL namespace.
//!
//! Each type is written by hand against the published definition rather than
//! generated, because six messages do not justify a DSDL compiler and a hand
//! written struct is what a reader of the crate wants to see. Every field keeps
//! its DSDL name and unit so a line of Rust can be checked against a line of
//! the definition without a mapping table.
//!
//! Field semantics are cross-checked against PX4's
//! `InternalCombustionEngineStatus` and ArduPilot's `EFI`/`EFI2`/`ECYL` log
//! messages, which are what actually consumes these on a real airframe.
//!
//! <https://github.com/dronecan/DSDL/tree/master/uavcan/equipment>

mod air_data;
mod dragonfly;
mod fuel_tank;
mod power;
mod reciprocating;

pub use air_data::{IndicatedAirspeed, StaticPressure, StaticTemperature};
pub use dragonfly::AuxiliaryStatus;
pub use fuel_tank::FuelTankStatus;
pub use power::{CircuitStatus, error_flags};
pub use reciprocating::{CylinderStatus, EngineState, ReciprocatingStatus, flags};

use crate::bits::{BitReader, BitWriter};
use crate::transfer::Transfer;

/// Why a payload could not be turned into a message.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum DecodeError {
    /// The payload ran out before every field had been read.
    #[error("{type_name}: payload of {len} bytes ended before the message did")]
    Truncated {
        /// Full DSDL name of the type being decoded.
        type_name: &'static str,
        /// Length of the payload that was offered.
        len: usize,
    },
    /// The transfer checksum disagrees with the payload.
    ///
    /// In practice this means the sender and this crate disagree about the shape
    /// of the data type, not that the bus corrupted anything: the CAN controller
    /// already checks each frame.
    #[error("{type_name}: transfer checksum mismatch, sender may use a different definition")]
    Checksum {
        /// Full DSDL name of the type being decoded.
        type_name: &'static str,
    },
    /// The transfer carries some other data type.
    #[error("expected data type {expected}, transfer carries {actual}")]
    DataTypeId {
        /// Data type ID the caller asked for.
        expected: u16,
        /// Data type ID the transfer actually carried.
        actual: u16,
    },
}

/// A top-level DSDL message.
pub trait Message: Sized {
    /// Full DSDL type name.
    const NAME: &'static str;
    /// Normalised definition, the input to the signature hash.
    const NORMALISED_DEFINITION: &'static str;
    /// Data type ID from the definition. Vendor types may be reassigned at runtime.
    const DEFAULT_DATA_TYPE_ID: u16;
    /// Data type signature, seeding the transfer checksum.
    const SIGNATURE: u64;

    /// Serialise the fields in definition order.
    fn encode_into(&self, w: &mut BitWriter);

    /// Deserialise the fields in definition order.
    fn decode_from(r: &mut BitReader<'_>) -> Option<Self>;

    /// Serialise to a payload ready to be split into frames.
    #[must_use]
    fn encode(&self) -> Vec<u8> {
        let mut w = BitWriter::new();
        self.encode_into(&mut w);
        w.finish()
    }

    /// Deserialise a payload.
    ///
    /// # Errors
    /// [`DecodeError::Truncated`] if the payload is shorter than the message.
    fn decode(payload: &[u8]) -> Result<Self, DecodeError> {
        let mut r = BitReader::new(payload);
        Self::decode_from(&mut r).ok_or(DecodeError::Truncated {
            type_name: Self::NAME,
            len: payload.len(),
        })
    }

    /// Deserialise a reassembled transfer, checking its data type and checksum first.
    ///
    /// # Errors
    /// [`DecodeError::DataTypeId`] if the transfer carries another type,
    /// [`DecodeError::Checksum`] if the payload fails the transfer checksum, or
    /// [`DecodeError::Truncated`] as for [`Message::decode`].
    fn from_transfer(transfer: &Transfer) -> Result<Self, DecodeError> {
        Self::from_transfer_as(transfer, Self::DEFAULT_DATA_TYPE_ID)
    }

    /// As [`Message::from_transfer`], but for a type whose ID has been reassigned.
    ///
    /// Vendor data type IDs live in a range with no registry and the
    /// specification requires the operator be able to change them, so a receiver
    /// cannot assume a vendor message arrives under the ID its definition names.
    ///
    /// # Errors
    /// As [`Message::from_transfer`].
    fn from_transfer_as(transfer: &Transfer, data_type_id: u16) -> Result<Self, DecodeError> {
        if transfer.id.data_type_id != data_type_id {
            return Err(DecodeError::DataTypeId {
                expected: data_type_id,
                actual: transfer.id.data_type_id,
            });
        }
        if !transfer.crc_ok(Self::SIGNATURE) {
            return Err(DecodeError::Checksum {
                type_name: Self::NAME,
            });
        }
        Self::decode(&transfer.payload)
    }
}
