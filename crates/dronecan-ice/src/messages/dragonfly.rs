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

/// Node ID the ground station commands from.
///
/// Low, and outside the block the simulated engine nodes occupy at 42 to 44, so
/// a capture makes the direction of a transfer obvious: anything from 1 is going
/// down to the aircraft and everything else is coming up from it.
pub const NODE_GROUND_STATION: u8 = 1;

/// Which fault a [`FaultCommand`] asks for.
///
/// The wire carries this as a `uint8`. An unknown value is ignored rather than
/// rejected, because a newer ground station talking to an older simulator should
/// leave the machine alone rather than stop it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum FaultKind {
    /// Remove every injected fault.
    Clear = 0,
    /// Injector nozzle coking, one cylinder.
    InjectorCoking = 1,
    /// Combustion failure, one cylinder.
    Misfire = 2,
    /// Exhaust probe reading progressively high, one cylinder.
    SensorDrift = 3,
    /// Exhaust probe holding its last sample, one cylinder.
    SensorFreeze = 4,
    /// Radiator fouling, engine wide.
    CoolingDegradation = 5,
}

impl FaultKind {
    /// The kind this byte names, or `None` for one this build does not know.
    #[must_use]
    pub fn from_u8(value: u8) -> Option<Self> {
        Some(match value {
            0 => Self::Clear,
            1 => Self::InjectorCoking,
            2 => Self::Misfire,
            3 => Self::SensorDrift,
            4 => Self::SensorFreeze,
            5 => Self::CoolingDegradation,
            _ => return None,
        })
    }
}

/// A ground station asking the simulator to inject a fault.
///
/// # This commands a simulator, never an engine
///
/// Nothing on a real aircraft would accept a message that damages it, and this
/// type must never be given a handler in flight software. It exists because a
/// demonstration has to be able to break the engine while somebody is watching,
/// and routing that over the bus rather than over a side channel keeps one
/// transport for everything and makes the CAN link visibly bidirectional.
///
/// # Repeated rather than acknowledged
///
/// CAN has no delivery guarantee and this crate implements no service protocol,
/// so a command is published several times and carries a `sequence` the receiver
/// de-duplicates on. That makes it idempotent: applying the same sequence twice
/// is a no-op, and losing two of three copies still lands the command.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FaultCommand {
    /// Increments per command, wrapping. Repeats of one command share it.
    pub sequence: u8,
    /// Which fault, as a [`FaultKind`] discriminant.
    pub kind: u8,
    /// Cylinder, 1 to 4. Zero for a fault that is not per-cylinder.
    pub cylinder: u8,
    /// Fault-specific magnitude. Fraction remaining for coking, K/h for drift.
    pub severity: f32,
    /// Seconds the fault takes to reach `severity` from the moment it lands.
    pub ramp_s: f32,
}

impl Message for FaultCommand {
    const NAME: &'static str = "dragonfly.FaultCommand";
    const NORMALISED_DEFINITION: &'static str = concat!(
        "dragonfly.FaultCommand\n",
        "saturated uint8 sequence\n",
        "saturated uint8 kind\n",
        "saturated uint8 cylinder\n",
        "saturated float16 severity\n",
        "saturated float16 ramp_s"
    );
    const DEFAULT_DATA_TYPE_ID: u16 = 20951;
    const SIGNATURE: u64 = dsdl_signature(Self::NORMALISED_DEFINITION);

    fn encode_into(&self, w: &mut BitWriter) {
        w.write_uint(u64::from(self.sequence), 8);
        w.write_uint(u64::from(self.kind), 8);
        w.write_uint(u64::from(self.cylinder), 8);
        w.write_f16(self.severity);
        w.write_f16(self.ramp_s);
    }

    fn decode_from(r: &mut BitReader<'_>) -> Option<Self> {
        Some(Self {
            sequence: u8::try_from(r.read_uint(8)?).ok()?,
            kind: u8::try_from(r.read_uint(8)?).ok()?,
            cylinder: u8::try_from(r.read_uint(8)?).ok()?,
            severity: r.read_f16()?,
            ramp_s: r.read_f16()?,
        })
    }
}

#[cfg(test)]
mod command_tests {
    use super::*;

    #[test]
    fn a_command_survives_a_round_trip() {
        let cmd = FaultCommand {
            sequence: 7,
            kind: FaultKind::InjectorCoking as u8,
            cylinder: 3,
            severity: 0.72,
            ramp_s: 120.0,
        };
        let back = FaultCommand::decode(&cmd.encode()).expect("decodes");
        assert_eq!(back.sequence, 7);
        assert_eq!(back.cylinder, 3);
        assert_eq!(back.ramp_s, 120.0);
        assert!((back.severity - 0.72).abs() < 1e-3, "half float, not exact");
    }

    /// Seven bytes, which is one CAN frame with its tail byte and no CRC. A
    /// command that needed two frames would need reassembly to land at all.
    #[test]
    fn a_command_fits_one_frame() {
        let cmd = FaultCommand {
            sequence: 1,
            kind: 1,
            cylinder: 1,
            severity: 1.0,
            ramp_s: 1.0,
        };
        assert_eq!(cmd.encode().len(), crate::transfer::BYTES_PER_FRAME);
    }

    #[test]
    fn an_unknown_kind_is_ignored_rather_than_guessed() {
        assert_eq!(FaultKind::from_u8(200), None);
        assert_eq!(FaultKind::from_u8(1), Some(FaultKind::InjectorCoking));
    }

    #[test]
    fn the_id_is_inside_the_vendor_range_and_is_not_the_status_id() {
        assert!((20_000..21_000).contains(&FaultCommand::DEFAULT_DATA_TYPE_ID));
        assert_ne!(
            FaultCommand::DEFAULT_DATA_TYPE_ID,
            AuxiliaryStatus::DEFAULT_DATA_TYPE_ID
        );
    }
}
