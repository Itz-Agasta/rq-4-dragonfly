//! `uavcan.equipment.air_data`, the three single-frame air data messages.
//!
//! Pressure altitude is not broadcast: static pressure is, and altitude is
//! derived from it. That is how a real air data computer publishes, and it means
//! the consumer applies its own atmosphere model rather than inheriting one.
//!
//! Each message pairs a value with its variance. A variance of NaN means the
//! sender does not characterise its own noise.
//!
//! <https://github.com/dronecan/DSDL/tree/master/uavcan/equipment/air_data>

use crate::bits::{BitReader, BitWriter};
use crate::messages::Message;
use crate::signature::dsdl_signature;

/// `uavcan.equipment.air_data.StaticPressure`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StaticPressure {
    /// Static pressure, Pa.
    pub static_pressure: f32,
    /// Variance of the above, Pa squared.
    pub static_pressure_variance: f32,
}

impl Message for StaticPressure {
    const NAME: &'static str = "uavcan.equipment.air_data.StaticPressure";
    const NORMALISED_DEFINITION: &'static str = concat!(
        "uavcan.equipment.air_data.StaticPressure\n",
        "saturated float32 static_pressure\n",
        "saturated float16 static_pressure_variance"
    );
    const DEFAULT_DATA_TYPE_ID: u16 = 1028;
    const SIGNATURE: u64 = dsdl_signature(Self::NORMALISED_DEFINITION);

    fn encode_into(&self, w: &mut BitWriter) {
        w.write_f32(self.static_pressure);
        w.write_f16(self.static_pressure_variance);
    }

    fn decode_from(r: &mut BitReader<'_>) -> Option<Self> {
        Some(Self {
            static_pressure: r.read_f32()?,
            static_pressure_variance: r.read_f16()?,
        })
    }
}

/// `uavcan.equipment.air_data.StaticTemperature`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StaticTemperature {
    /// Outside air temperature, K.
    pub static_temperature: f32,
    /// Variance of the above, K squared.
    pub static_temperature_variance: f32,
}

impl Message for StaticTemperature {
    const NAME: &'static str = "uavcan.equipment.air_data.StaticTemperature";
    const NORMALISED_DEFINITION: &'static str = concat!(
        "uavcan.equipment.air_data.StaticTemperature\n",
        "saturated float16 static_temperature\n",
        "saturated float16 static_temperature_variance"
    );
    const DEFAULT_DATA_TYPE_ID: u16 = 1029;
    const SIGNATURE: u64 = dsdl_signature(Self::NORMALISED_DEFINITION);

    fn encode_into(&self, w: &mut BitWriter) {
        w.write_f16(self.static_temperature);
        w.write_f16(self.static_temperature_variance);
    }

    fn decode_from(r: &mut BitReader<'_>) -> Option<Self> {
        Some(Self {
            static_temperature: r.read_f16()?,
            static_temperature_variance: r.read_f16()?,
        })
    }
}

/// `uavcan.equipment.air_data.IndicatedAirspeed`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IndicatedAirspeed {
    /// Indicated airspeed, m/s.
    pub indicated_airspeed: f32,
    /// Variance of the above, (m/s) squared.
    pub indicated_airspeed_variance: f32,
}

impl Message for IndicatedAirspeed {
    const NAME: &'static str = "uavcan.equipment.air_data.IndicatedAirspeed";
    const NORMALISED_DEFINITION: &'static str = concat!(
        "uavcan.equipment.air_data.IndicatedAirspeed\n",
        "saturated float16 indicated_airspeed\n",
        "saturated float16 indicated_airspeed_variance"
    );
    const DEFAULT_DATA_TYPE_ID: u16 = 1021;
    const SIGNATURE: u64 = dsdl_signature(Self::NORMALISED_DEFINITION);

    fn encode_into(&self, w: &mut BitWriter) {
        w.write_f16(self.indicated_airspeed);
        w.write_f16(self.indicated_airspeed_variance);
    }

    fn decode_from(r: &mut BitReader<'_>) -> Option<Self> {
        Some(Self {
            indicated_airspeed: r.read_f16()?,
            indicated_airspeed_variance: r.read_f16()?,
        })
    }
}
