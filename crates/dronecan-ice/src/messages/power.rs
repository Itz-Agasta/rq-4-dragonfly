//! `uavcan.equipment.power.CircuitStatus`.
//!
//! Chosen over `BatteryInfo` for reporting a bus: 56 bits fits one frame where
//! `BatteryInfo` needs 437 and most of its fields describe a cell chemistry that
//! an engine-driven alternator does not have.
//!
//! <https://github.com/dronecan/DSDL/blob/master/uavcan/equipment/power/1091.CircuitStatus.uavcan>

use crate::bits::{BitReader, BitWriter};
use crate::messages::Message;
use crate::signature::dsdl_signature;

/// Voltage and current on one electrical circuit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CircuitStatus {
    /// Which circuit this is.
    pub circuit_id: u16,
    /// Bus voltage, V.
    pub voltage: f32,
    /// Bus current, A.
    pub current: f32,
    /// Bitmask from [`error_flags`].
    pub error_flags: u8,
}

/// Bits carried in [`CircuitStatus::error_flags`].
pub mod error_flags {
    /// Voltage above its nominal band.
    pub const OVERVOLTAGE: u8 = 1;
    /// Voltage below its nominal band.
    pub const UNDERVOLTAGE: u8 = 2;
    /// Current above its nominal band.
    pub const OVERCURRENT: u8 = 4;
    /// Current below its nominal band.
    pub const UNDERCURRENT: u8 = 8;
}

impl Message for CircuitStatus {
    const NAME: &'static str = "uavcan.equipment.power.CircuitStatus";
    const NORMALISED_DEFINITION: &'static str = concat!(
        "uavcan.equipment.power.CircuitStatus\n",
        "saturated uint16 circuit_id\n",
        "saturated float16 voltage\n",
        "saturated float16 current\n",
        "saturated uint8 error_flags"
    );
    const DEFAULT_DATA_TYPE_ID: u16 = 1091;
    const SIGNATURE: u64 = dsdl_signature(Self::NORMALISED_DEFINITION);

    fn encode_into(&self, w: &mut BitWriter) {
        w.write_uint(u64::from(self.circuit_id), 16);
        w.write_f16(self.voltage);
        w.write_f16(self.current);
        w.write_uint(u64::from(self.error_flags), 8);
    }

    fn decode_from(r: &mut BitReader<'_>) -> Option<Self> {
        Some(Self {
            circuit_id: r.read_uint(16)? as u16,
            voltage: r.read_f16()?,
            current: r.read_f16()?,
            error_flags: r.read_uint(8)? as u8,
        })
    }
}
