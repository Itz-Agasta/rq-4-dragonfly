//! `uavcan.equipment.ice.FuelTankStatus`.
//!
//! <https://github.com/dronecan/DSDL/blob/master/uavcan/equipment/ice/1129.FuelTankStatus.uavcan>

use crate::bits::{BitReader, BitWriter};
use crate::messages::Message;
use crate::signature::dsdl_signature;

/// Remaining fuel and the rate it is going.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FuelTankStatus {
    /// Fuel remaining, percent, 0 to 127.
    pub available_fuel_volume_percent: u8,
    /// Fuel remaining, cm3.
    pub available_fuel_volume_cm3: f32,
    /// Consumption rate, cm3/min. Negative while transferring between tanks.
    pub fuel_consumption_rate_cm3pm: f32,
    /// Fuel temperature, K. Optional; NaN if not measured.
    pub fuel_temperature: f32,
    /// Which tank this is.
    pub fuel_tank_id: u8,
}

impl Default for FuelTankStatus {
    fn default() -> Self {
        Self {
            available_fuel_volume_percent: 0,
            available_fuel_volume_cm3: 0.0,
            fuel_consumption_rate_cm3pm: 0.0,
            fuel_temperature: f32::NAN,
            fuel_tank_id: 0,
        }
    }
}

impl Message for FuelTankStatus {
    const NAME: &'static str = "uavcan.equipment.ice.FuelTankStatus";
    const NORMALISED_DEFINITION: &'static str = concat!(
        "uavcan.equipment.ice.FuelTankStatus\n",
        "void9\n",
        "saturated uint7 available_fuel_volume_percent\n",
        "saturated float32 available_fuel_volume_cm3\n",
        "saturated float32 fuel_consumption_rate_cm3pm\n",
        "saturated float16 fuel_temperature\n",
        "saturated uint8 fuel_tank_id"
    );
    const DEFAULT_DATA_TYPE_ID: u16 = 1129;
    const SIGNATURE: u64 = dsdl_signature(Self::NORMALISED_DEFINITION);

    fn encode_into(&self, w: &mut BitWriter) {
        w.write_void(9);
        w.write_uint(u64::from(self.available_fuel_volume_percent), 7);
        w.write_f32(self.available_fuel_volume_cm3);
        w.write_f32(self.fuel_consumption_rate_cm3pm);
        w.write_f16(self.fuel_temperature);
        w.write_uint(u64::from(self.fuel_tank_id), 8);
    }

    fn decode_from(r: &mut BitReader<'_>) -> Option<Self> {
        r.skip_void(9)?;
        Some(Self {
            available_fuel_volume_percent: r.read_uint(7)? as u8,
            available_fuel_volume_cm3: r.read_f32()?,
            fuel_consumption_rate_cm3pm: r.read_f32()?,
            fuel_temperature: r.read_f16()?,
            fuel_tank_id: r.read_uint(8)? as u8,
        })
    }
}
