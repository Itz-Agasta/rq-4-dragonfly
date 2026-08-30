//! `uavcan.equipment.ice.reciprocating`, the engine status message set.
//!
//! Written for a spark-ignition engine, which shows in the field list: there is a
//! spark dwell time, a spark plug usage enum and a throttle position, none of
//! which a common-rail compression-ignition engine has. The convention ArduPilot
//! and PX4 follow is to leave the spark fields at NaN and report fuelling demand
//! through `throttle_position_percent`, and this crate does the same rather than
//! inventing a parallel message.
//!
//! `cylinder_status` is the last field of the top-level type and its item is 80
//! bits, so the tail array optimisation applies: the five-bit length prefix is
//! **omitted** and the count is recovered from the transfer length. The published
//! maximum bit length of 1565 includes that prefix and is therefore not the
//! length of anything on the wire.
//!
//! <https://github.com/dronecan/DSDL/blob/master/uavcan/equipment/ice/reciprocating/1120.Status.uavcan>

use crate::bits::{BitReader, BitWriter};
use crate::messages::Message;
use crate::signature::{dsdl_signature, extend};

/// General status flags carried in the `flags` field.
///
/// Optional groups are gated by a `_SUPPORTED` bit: with it clear, the rest of
/// the group must be ignored rather than read as "condition absent".
pub mod flags {
    /// An error not covered by any other flag. Always meaningful.
    pub const GENERAL_ERROR: u32 = 1;
    /// Crankshaft sensor group is populated.
    pub const CRANKSHAFT_SENSOR_ERROR_SUPPORTED: u32 = 2;
    /// Crankshaft sensor has failed.
    pub const CRANKSHAFT_SENSOR_ERROR: u32 = 4;
    /// Temperature group is populated.
    pub const TEMPERATURE_SUPPORTED: u32 = 8;
    /// Under-temperature warning.
    pub const TEMPERATURE_BELOW_NOMINAL: u32 = 16;
    /// Over-temperature warning.
    pub const TEMPERATURE_ABOVE_NOMINAL: u32 = 32;
    /// Critical overheating.
    pub const TEMPERATURE_OVERHEATING: u32 = 64;
    /// Exhaust gas over-temperature warning.
    pub const TEMPERATURE_EGT_ABOVE_NOMINAL: u32 = 128;
    /// Fuel pressure group is populated.
    pub const FUEL_PRESSURE_SUPPORTED: u32 = 256;
    /// Fuel under-pressure warning.
    pub const FUEL_PRESSURE_BELOW_NOMINAL: u32 = 512;
    /// Fuel over-pressure warning.
    pub const FUEL_PRESSURE_ABOVE_NOMINAL: u32 = 1024;
    /// Detonation detection is available.
    pub const DETONATION_SUPPORTED: u32 = 2048;
    /// Detonation observed.
    pub const DETONATION_OBSERVED: u32 = 4096;
    /// Misfire detection is available.
    pub const MISFIRE_SUPPORTED: u32 = 8192;
    /// Misfire observed.
    pub const MISFIRE_OBSERVED: u32 = 16384;
    /// Oil pressure group is populated.
    pub const OIL_PRESSURE_SUPPORTED: u32 = 32768;
    /// Oil under-pressure warning.
    pub const OIL_PRESSURE_BELOW_NOMINAL: u32 = 65536;
    /// Oil over-pressure warning.
    pub const OIL_PRESSURE_ABOVE_NOMINAL: u32 = 131_072;
    /// Debris detection is available.
    pub const DEBRIS_SUPPORTED: u32 = 262_144;
    /// Debris detected in the oil.
    pub const DEBRIS_DETECTED: u32 = 524_288;
}

/// Abstract engine state, the `state` field.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum EngineState {
    /// Not running. The default.
    #[default]
    Stopped = 0,
    /// Cranking or starting. Transient.
    Starting = 1,
    /// Running normally. Error flags may still be set for non-fatal conditions.
    Running = 2,
    /// Can no longer function.
    Fault = 3,
}

impl EngineState {
    const fn from_bits(raw: u64) -> Self {
        match raw & 0x3 {
            0 => Self::Stopped,
            1 => Self::Starting,
            2 => Self::Running,
            _ => Self::Fault,
        }
    }
}

/// Per-cylinder measurements, the nested `CylinderStatus` type.
///
/// Every field is optional and NaN means "not instrumented". A shared exhaust
/// probe is reported by giving every cylinder the same temperature rather than by
/// populating one and leaving the rest NaN.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CylinderStatus {
    /// Ignition timing, crankshaft degrees. NaN on a compression-ignition engine.
    pub ignition_timing_deg: f32,
    /// Fuel injection duration, ms.
    pub injection_time_ms: f32,
    /// Cylinder head temperature, K.
    pub cylinder_head_temperature: f32,
    /// Exhaust gas temperature, K.
    pub exhaust_gas_temperature: f32,
    /// Excess air ratio. Runs well above unity on a diesel.
    pub lambda_coefficient: f32,
}

impl Default for CylinderStatus {
    /// Every field NaN, which is the specified way to say "unknown".
    fn default() -> Self {
        Self {
            ignition_timing_deg: f32::NAN,
            injection_time_ms: f32::NAN,
            cylinder_head_temperature: f32::NAN,
            exhaust_gas_temperature: f32::NAN,
            lambda_coefficient: f32::NAN,
        }
    }
}

impl CylinderStatus {
    /// Full DSDL type name.
    pub const NAME: &'static str = "uavcan.equipment.ice.reciprocating.CylinderStatus";
    /// Normalised definition, the input to the signature hash.
    pub const NORMALISED_DEFINITION: &'static str = concat!(
        "uavcan.equipment.ice.reciprocating.CylinderStatus\n",
        "saturated float16 ignition_timing_deg\n",
        "saturated float16 injection_time_ms\n",
        "saturated float16 cylinder_head_temperature\n",
        "saturated float16 exhaust_gas_temperature\n",
        "saturated float16 lambda_coefficient"
    );
    /// Data type signature of the nested type.
    pub const SIGNATURE: u64 = dsdl_signature(Self::NORMALISED_DEFINITION);
    /// Serialised width. Fixed, which is what makes the tail array unambiguous.
    pub const BITS: usize = 80;

    fn encode_into(&self, w: &mut BitWriter) {
        w.write_f16(self.ignition_timing_deg);
        w.write_f16(self.injection_time_ms);
        w.write_f16(self.cylinder_head_temperature);
        w.write_f16(self.exhaust_gas_temperature);
        w.write_f16(self.lambda_coefficient);
    }

    fn decode_from(r: &mut BitReader<'_>) -> Option<Self> {
        Some(Self {
            ignition_timing_deg: r.read_f16()?,
            injection_time_ms: r.read_f16()?,
            cylinder_head_temperature: r.read_f16()?,
            exhaust_gas_temperature: r.read_f16()?,
            lambda_coefficient: r.read_f16()?,
        })
    }
}

/// Largest `cylinder_status` array the definition allows.
pub const MAX_CYLINDERS: usize = 16;

/// `uavcan.equipment.ice.reciprocating.Status`.
///
/// Integer fields are required; floating point fields are optional and NaN is how
/// a controller says it does not measure one.
#[derive(Clone, Debug, PartialEq)]
pub struct ReciprocatingStatus {
    /// Abstract engine state.
    pub state: EngineState,
    /// Bitmask from [`flags`].
    pub flags: u32,
    /// Engine load estimate, percent, 0 to 127.
    pub engine_load_percent: u8,
    /// Crankshaft speed, rpm. 17 bits.
    pub engine_speed_rpm: u32,
    /// Spark dwell, ms. NaN on a compression-ignition engine.
    pub spark_dwell_time_ms: f32,
    /// Ambient static pressure, kPa.
    pub atmospheric_pressure_kpa: f32,
    /// Intake manifold absolute pressure, kPa.
    pub intake_manifold_pressure_kpa: f32,
    /// Intake manifold charge temperature, K.
    pub intake_manifold_temperature: f32,
    /// Coolant temperature, K.
    pub coolant_temperature: f32,
    /// Oil pressure, kPa.
    pub oil_pressure: f32,
    /// Oil temperature, K.
    pub oil_temperature: f32,
    /// Fuel rail pressure, kPa.
    pub fuel_pressure: f32,
    /// Instantaneous fuel consumption, cm3/min. Should be low-pass filtered.
    pub fuel_consumption_rate_cm3pm: f32,
    /// Fuel burnt since the engine started, cm3. Reset on stop.
    pub estimated_consumed_fuel_volume_cm3: f32,
    /// Throttle position, percent, 0 to 127. Fuelling demand on a diesel.
    pub throttle_position_percent: u8,
    /// Index of the publishing engine control unit, 0 to 63.
    pub ecu_index: u8,
    /// Spark plug activity, 0 to 7. Meaningless without spark ignition.
    pub spark_plug_usage: u8,
    /// Per-cylinder measurements, at most [`MAX_CYLINDERS`].
    pub cylinder_status: Vec<CylinderStatus>,
}

impl Default for ReciprocatingStatus {
    /// Stopped, no flags, every optional measurement NaN.
    fn default() -> Self {
        Self {
            state: EngineState::Stopped,
            flags: 0,
            engine_load_percent: 0,
            engine_speed_rpm: 0,
            spark_dwell_time_ms: f32::NAN,
            atmospheric_pressure_kpa: f32::NAN,
            intake_manifold_pressure_kpa: f32::NAN,
            intake_manifold_temperature: f32::NAN,
            coolant_temperature: f32::NAN,
            oil_pressure: f32::NAN,
            oil_temperature: f32::NAN,
            fuel_pressure: f32::NAN,
            fuel_consumption_rate_cm3pm: f32::NAN,
            estimated_consumed_fuel_volume_cm3: f32::NAN,
            throttle_position_percent: 0,
            ecu_index: 0,
            spark_plug_usage: 0,
            cylinder_status: Vec::new(),
        }
    }
}

impl Message for ReciprocatingStatus {
    const NAME: &'static str = "uavcan.equipment.ice.reciprocating.Status";
    const NORMALISED_DEFINITION: &'static str = concat!(
        "uavcan.equipment.ice.reciprocating.Status\n",
        "saturated uint2 state\n",
        "saturated uint30 flags\n",
        "void16\n",
        "saturated uint7 engine_load_percent\n",
        "saturated uint17 engine_speed_rpm\n",
        "saturated float16 spark_dwell_time_ms\n",
        "saturated float16 atmospheric_pressure_kpa\n",
        "saturated float16 intake_manifold_pressure_kpa\n",
        "saturated float16 intake_manifold_temperature\n",
        "saturated float16 coolant_temperature\n",
        "saturated float16 oil_pressure\n",
        "saturated float16 oil_temperature\n",
        "saturated float16 fuel_pressure\n",
        "saturated float32 fuel_consumption_rate_cm3pm\n",
        "saturated float32 estimated_consumed_fuel_volume_cm3\n",
        "saturated uint7 throttle_position_percent\n",
        "saturated uint6 ecu_index\n",
        "saturated uint3 spark_plug_usage\n",
        "uavcan.equipment.ice.reciprocating.CylinderStatus[<=16] cylinder_status"
    );
    const DEFAULT_DATA_TYPE_ID: u16 = 1120;
    const SIGNATURE: u64 = extend(
        dsdl_signature(Self::NORMALISED_DEFINITION),
        CylinderStatus::SIGNATURE,
    );

    fn encode_into(&self, w: &mut BitWriter) {
        w.write_uint(self.state as u64, 2);
        w.write_uint(u64::from(self.flags), 30);
        w.write_void(16);
        w.write_uint(u64::from(self.engine_load_percent), 7);
        w.write_uint(u64::from(self.engine_speed_rpm), 17);
        w.write_f16(self.spark_dwell_time_ms);
        w.write_f16(self.atmospheric_pressure_kpa);
        w.write_f16(self.intake_manifold_pressure_kpa);
        w.write_f16(self.intake_manifold_temperature);
        w.write_f16(self.coolant_temperature);
        w.write_f16(self.oil_pressure);
        w.write_f16(self.oil_temperature);
        w.write_f16(self.fuel_pressure);
        w.write_f32(self.fuel_consumption_rate_cm3pm);
        w.write_f32(self.estimated_consumed_fuel_volume_cm3);
        w.write_uint(u64::from(self.throttle_position_percent), 7);
        w.write_uint(u64::from(self.ecu_index), 6);
        w.write_uint(u64::from(self.spark_plug_usage), 3);
        for cylinder in self.cylinder_status.iter().take(MAX_CYLINDERS) {
            cylinder.encode_into(w);
        }
    }

    fn decode_from(r: &mut BitReader<'_>) -> Option<Self> {
        let state = EngineState::from_bits(r.read_uint(2)?);
        let flags = r.read_uint(30)? as u32;
        r.skip_void(16)?;
        let engine_load_percent = r.read_uint(7)? as u8;
        let engine_speed_rpm = r.read_uint(17)? as u32;
        let spark_dwell_time_ms = r.read_f16()?;
        let atmospheric_pressure_kpa = r.read_f16()?;
        let intake_manifold_pressure_kpa = r.read_f16()?;
        let intake_manifold_temperature = r.read_f16()?;
        let coolant_temperature = r.read_f16()?;
        let oil_pressure = r.read_f16()?;
        let oil_temperature = r.read_f16()?;
        let fuel_pressure = r.read_f16()?;
        let fuel_consumption_rate_cm3pm = r.read_f32()?;
        let estimated_consumed_fuel_volume_cm3 = r.read_f32()?;
        let throttle_position_percent = r.read_uint(7)? as u8;
        let ecu_index = r.read_uint(6)? as u8;
        let spark_plug_usage = r.read_uint(3)? as u8;

        // Tail array: no length prefix on the wire, so the count is whatever the
        // transfer length leaves room for.
        let count = (r.remaining() / CylinderStatus::BITS).min(MAX_CYLINDERS);
        let mut cylinder_status = Vec::with_capacity(count);
        for _ in 0..count {
            cylinder_status.push(CylinderStatus::decode_from(r)?);
        }

        Some(Self {
            state,
            flags,
            engine_load_percent,
            engine_speed_rpm,
            spark_dwell_time_ms,
            atmospheric_pressure_kpa,
            intake_manifold_pressure_kpa,
            intake_manifold_temperature,
            coolant_temperature,
            oil_pressure,
            oil_temperature,
            fuel_pressure,
            fuel_consumption_rate_cm3pm,
            estimated_consumed_fuel_volume_cm3,
            throttle_position_percent,
            ecu_index,
            spark_plug_usage,
            cylinder_status,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The tail array is the one place where getting the length wrong still
    /// produces a payload that decodes, so the width is asserted rather than
    /// trusted: 35 bytes fixed plus 10 per cylinder, and no length prefix.
    #[test]
    fn four_cylinders_serialise_to_seventy_five_bytes() {
        let status = ReciprocatingStatus {
            cylinder_status: vec![CylinderStatus::default(); 4],
            ..ReciprocatingStatus::default()
        };
        // 280 bits of fixed fields is 35 bytes, plus 10 per cylinder. A length
        // prefix would make it 76 and every field after it would shift.
        assert_eq!(status.encode().len(), 75);
    }

    #[test]
    fn a_status_round_trips() {
        let status = ReciprocatingStatus {
            state: EngineState::Running,
            flags: flags::TEMPERATURE_SUPPORTED | flags::OIL_PRESSURE_SUPPORTED,
            engine_load_percent: 34,
            engine_speed_rpm: 3720,
            coolant_temperature: 361.0,
            oil_pressure: 420.0,
            cylinder_status: vec![CylinderStatus {
                cylinder_head_temperature: 412.0,
                exhaust_gas_temperature: 843.0,
                ..CylinderStatus::default()
            }],
            ..ReciprocatingStatus::default()
        };
        let back = ReciprocatingStatus::decode(&status.encode()).expect("decodes");
        assert_eq!(back.state, EngineState::Running);
        assert_eq!(back.flags, status.flags);
        assert_eq!(back.engine_speed_rpm, 3720);
        assert_eq!(back.cylinder_status.len(), 1);
        assert!((back.cylinder_status[0].exhaust_gas_temperature - 843.0).abs() < 1.0);
        assert!(back.spark_dwell_time_ms.is_nan());
    }

    #[test]
    fn a_truncated_payload_is_an_error_not_a_panic() {
        assert!(ReciprocatingStatus::decode(&[0u8; 10]).is_err());
    }
}
