//! `dragonfly.AuxiliaryStatus`, the one message here that is not standard.
//!
//! Five quantities a piston-engine health monitor needs and that no standard
//! DroneCAN message carries. Everything else this crate handles travels on a
//! published definition; these do not exist in one, so rather than bend a
//! standard message they get a vendor type in a vendor namespace, which is what
//! the specification requires. Defining vendor types inside `uavcan` is
//! prohibited.
//!
//! Turbocharger speed is `float32` and not `float16` because a small
//! turbocharger runs past 100,000 rpm, above the largest finite binary16 value
//! of 65504. Encoded as a half it becomes infinity, silently, at every operating
//! point that matters.
//!
//! The data type ID is in the vendor range `[20000, 21000)` and, per the
//! specification, must be reconfigurable by the operator: it is a collision-prone
//! range with no central registry, so both binaries take it as an argument and
//! [`AuxiliaryStatus::DEFAULT_DATA_TYPE_ID`] is only a default.
//!
//! <https://dronecan.github.io/Specification/5._Application_level_conventions/#id-distribution>

use crate::bits::{BitReader, BitWriter};
use crate::messages::Message;
use crate::signature::dsdl_signature;

/// Engine quantities with no standard DroneCAN message.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AuxiliaryStatus {
    /// Turbocharger shaft speed, rpm.
    pub turbocharger_speed_rpm: f32,
    /// Mass air flow into the cylinders, kg/s.
    pub mass_air_flow_kgps: f32,
    /// Wastegate position, 0 shut to 1 fully open.
    ///
    /// Published because a twin that has it can run the engine model open loop
    /// against the real actuator; without it the twin has to guess what the boost
    /// controller did, and a wrong guess shows up as a boost residual that looks
    /// like turbocharger degradation.
    pub wastegate_position: f32,
    /// Broadband vibration, g RMS.
    pub vibration_rms_g: f32,
    /// Kurtosis of the same vibration signal, dimensionless.
    ///
    /// Reported alongside RMS rather than instead of it because kurtosis rises
    /// first for an impulsive defect while RMS is still flat.
    pub vibration_kurtosis: f32,
}

impl Default for AuxiliaryStatus {
    fn default() -> Self {
        Self {
            turbocharger_speed_rpm: f32::NAN,
            mass_air_flow_kgps: f32::NAN,
            wastegate_position: f32::NAN,
            vibration_rms_g: f32::NAN,
            vibration_kurtosis: f32::NAN,
        }
    }
}

impl Message for AuxiliaryStatus {
    const NAME: &'static str = "dragonfly.AuxiliaryStatus";
    const NORMALISED_DEFINITION: &'static str = concat!(
        "dragonfly.AuxiliaryStatus\n",
        "saturated float32 turbocharger_speed_rpm\n",
        "saturated float16 mass_air_flow_kgps\n",
        "saturated float16 wastegate_position\n",
        "saturated float16 vibration_rms_g\n",
        "saturated float16 vibration_kurtosis"
    );
    const DEFAULT_DATA_TYPE_ID: u16 = 20950;
    const SIGNATURE: u64 = dsdl_signature(Self::NORMALISED_DEFINITION);

    fn encode_into(&self, w: &mut BitWriter) {
        w.write_f32(self.turbocharger_speed_rpm);
        w.write_f16(self.mass_air_flow_kgps);
        w.write_f16(self.wastegate_position);
        w.write_f16(self.vibration_rms_g);
        w.write_f16(self.vibration_kurtosis);
    }

    fn decode_from(r: &mut BitReader<'_>) -> Option<Self> {
        Some(Self {
            turbocharger_speed_rpm: r.read_f32()?,
            mass_air_flow_kgps: r.read_f16()?,
            wastegate_position: r.read_f16()?,
            vibration_rms_g: r.read_f16()?,
            vibration_kurtosis: r.read_f16()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The reason this field is not a half float, asserted so nobody narrows it.
    #[test]
    fn turbocharger_speed_survives_the_encoding() {
        let aux = AuxiliaryStatus {
            turbocharger_speed_rpm: 118_400.0,
            ..AuxiliaryStatus::default()
        };
        let back = AuxiliaryStatus::decode(&aux.encode()).expect("decodes");
        assert_eq!(back.turbocharger_speed_rpm, 118_400.0);
        assert!(crate::bits::f32_from_f16(crate::bits::f16_from_f32(118_400.0)).is_infinite());
    }

    #[test]
    fn the_id_is_inside_the_vendor_range() {
        assert!((20_000..21_000).contains(&AuxiliaryStatus::DEFAULT_DATA_TYPE_ID));
    }
}
