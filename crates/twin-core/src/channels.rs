//! What the twin compares, and what it is willing to call a disagreement.
//!
//! One table describes every compared channel: where it sits in the measurement
//! vector, the magnitude it is normally at, and the two standard deviations that
//! together say how much residual is unremarkable.
//!
//! # Why there are two standard deviations and not one
//!
//! A residual is the sum of what the instrument got wrong and what the model got
//! wrong, and on a mean value engine model the second is much the larger. Sizing
//! the measurement covariance from instrument noise alone produces a filter that is
//! astonished by its own modelling error: every channel sits many standard
//! deviations out, the estimator drives the health parameters to absorb it, and the
//! whole point of separating degradation from model error is lost.
//!
//! The model term here is the accuracy claimed against a **real** engine, not
//! against a simulator sharing the same equations. Sizing it from a simulator would
//! give a residual band no physical installation could ever meet, which would be a
//! calibration that only works in the laboratory it was measured in.

use engine_model::CYLINDERS;

/// Index of each channel in the measurement vector.
pub mod index {
    /// Crankshaft speed.
    pub const RPM: usize = 0;
    /// Intake manifold pressure.
    pub const MAP: usize = 1;
    /// Intake manifold temperature.
    pub const MAT: usize = 2;
    /// Air mass flow.
    pub const MAF: usize = 3;
    /// Turbocharger shaft speed.
    pub const TURBO: usize = 4;
    /// Brake torque at the crankshaft.
    pub const TORQUE: usize = 5;
    /// Fuel mass flow.
    pub const FUEL_FLOW: usize = 6;
    /// Oil gallery pressure.
    pub const OIL_PRESSURE: usize = 7;
    /// Oil temperature.
    pub const OIL_TEMPERATURE: usize = 8;
    /// Coolant temperature.
    pub const COOLANT: usize = 9;
    /// First exhaust gas temperature; the four are contiguous.
    pub const EGT: usize = 10;
    /// First cylinder head temperature; the four are contiguous.
    pub const CHT: usize = 14;
    /// First excess air ratio; the four are contiguous.
    pub const LAMBDA: usize = 18;
}

/// How many channels are compared.
pub const CHANNELS: usize = 22;

/// One compared channel.
#[derive(Clone, Copy, Debug)]
pub struct Channel {
    /// Short name, as it appears in a readout.
    pub name: &'static str,
    /// Magnitude this channel normally sits at.
    ///
    /// Used to express a residual as a percentage without dividing by a
    /// measurement that may be near zero. A fixed reference also keeps the
    /// synchronisation quality comparable between operating points, where dividing
    /// by the live value would make the same absolute error read differently at
    /// idle and at full power.
    pub reference: f64,
    /// Instrument noise, one standard deviation.
    pub sensor_sigma: f64,
    /// Model accuracy against a real engine, one standard deviation.
    pub model_sigma: f64,
}

impl Channel {
    /// Total measurement standard deviation.
    #[must_use]
    pub fn sigma(&self) -> f64 {
        self.sensor_sigma.hypot(self.model_sigma)
    }

    /// Measurement variance, which is what the filter wants.
    #[must_use]
    pub fn variance(&self) -> f64 {
        let s = self.sigma();
        s * s
    }
}

/// Every compared channel, in measurement vector order.
///
/// Reference magnitudes are the canonical cruise point: 3720 rpm at 22,400 ft on a
/// standard day. Instrument noise is the specification a production engine
/// controller quotes for each channel, including the quantisation of the fields
/// that are broadcast as integers. Engine load is a whole percent of the rating,
/// which alone is most of the torque channel's noise.
pub const TABLE: [Channel; CHANNELS] = [
    Channel {
        name: "RPM",
        reference: 3720.0,
        sensor_sigma: 1.6,
        model_sigma: 5.0,
    },
    Channel {
        name: "MAP",
        reference: 130_200.0,
        sensor_sigma: 258.0,
        model_sigma: 1500.0,
    },
    Channel {
        name: "MAT",
        reference: 320.0,
        sensor_sigma: 0.8,
        model_sigma: 4.0,
    },
    Channel {
        name: "MAF",
        reference: 0.095,
        sensor_sigma: 3.0e-4,
        model_sigma: 1.5e-3,
    },
    Channel {
        name: "TURBO",
        reference: 122_100.0,
        sensor_sigma: 200.0,
        model_sigma: 1500.0,
    },
    Channel {
        name: "TORQUE",
        reference: 113.0,
        sensor_sigma: 1.0,
        model_sigma: 2.0,
    },
    Channel {
        name: "FUEL",
        reference: 11.5,
        sensor_sigma: 0.05,
        model_sigma: 0.15,
    },
    Channel {
        name: "OIL P",
        reference: 447_000.0,
        sensor_sigma: 1500.0,
        model_sigma: 8000.0,
    },
    Channel {
        name: "OIL T",
        reference: 356.0,
        sensor_sigma: 0.8,
        model_sigma: 3.0,
    },
    Channel {
        name: "COOLANT",
        reference: 358.0,
        sensor_sigma: 0.8,
        model_sigma: 3.0,
    },
    exhaust(1),
    exhaust(2),
    exhaust(3),
    exhaust(4),
    head(1),
    head(2),
    head(3),
    head(4),
    excess_air(1),
    excess_air(2),
    excess_air(3),
    excess_air(4),
];

/// One exhaust gas temperature channel.
///
/// The model standard deviation is the largest in the table and it is the one that
/// matters most: exhaust temperature is where an injection fault shows first, so it
/// sets what counts as evidence. A mean value model reproduces a cylinder-out
/// temperature to something like two percent of absolute, and 13 K on 750 K is that.
const fn exhaust(cylinder: u8) -> Channel {
    Channel {
        name: match cylinder {
            1 => "EGT 1",
            2 => "EGT 2",
            3 => "EGT 3",
            _ => "EGT 4",
        },
        reference: 752.0,
        sensor_sigma: 1.6,
        model_sigma: 12.9,
    }
}

/// One cylinder head temperature channel.
const fn head(cylinder: u8) -> Channel {
    Channel {
        name: match cylinder {
            1 => "CHT 1",
            2 => "CHT 2",
            3 => "CHT 3",
            _ => "CHT 4",
        },
        reference: 407.0,
        sensor_sigma: 0.8,
        model_sigma: 4.0,
    }
}

/// One per-cylinder excess air ratio channel.
///
/// There is no probe in each runner on a real engine. Per-cylinder excess air ratio
/// is computed by the controller from its own air and fuel estimates, so its
/// uncertainty is theirs compounded and is several percent, not the resolution of
/// the field it is broadcast in. Taking the broadcast number at face value would
/// make this the most certain channel on the bus by an order of magnitude, and a
/// fault would be reported at a departure no instrument could support.
const fn excess_air(cylinder: u8) -> Channel {
    Channel {
        name: match cylinder {
            1 => "LAMBDA 1",
            2 => "LAMBDA 2",
            3 => "LAMBDA 3",
            _ => "LAMBDA 4",
        },
        reference: 2.04,
        sensor_sigma: 0.01,
        model_sigma: 0.08,
    }
}

/// One instant of everything the twin is fed, in SI units.
///
/// Defined here rather than taken from the telemetry crate so this crate stays
/// independent of the wire format: the filter is testable against a scripted
/// mission with no bus, no daemon and no serialisation anywhere near it.
#[derive(Clone, Copy, Debug)]
pub struct Measurement {
    /// Seconds since the start of the run.
    pub t_s: f64,
    /// Ambient static pressure, Pa.
    pub p_amb_pa: f64,
    /// Outside air temperature, K.
    pub oat_k: f64,
    /// Indicated airspeed, m/s.
    pub ias_m_s: f64,
    /// Wastegate position, 0 shut to 1 open.
    pub wastegate: f64,
    /// Commanded injection duration, ms. The fuelling command as broadcast.
    pub injection_ms: f64,
    /// Crankshaft speed, rpm.
    pub rpm: f64,
    /// Intake manifold pressure, Pa.
    pub map_pa: f64,
    /// Intake manifold temperature, K.
    pub mat_k: f64,
    /// Air mass flow, kg/s.
    pub maf_kg_s: f64,
    /// Turbocharger shaft speed, rpm.
    pub turbo_rpm: f64,
    /// Brake torque at the crankshaft, N.m.
    pub torque_nm: f64,
    /// Fuel mass flow, kg/h.
    pub fuel_flow_kg_h: f64,
    /// Oil gallery pressure, Pa.
    pub oil_p_pa: f64,
    /// Oil temperature, K.
    pub oil_t_k: f64,
    /// Coolant temperature, K.
    pub coolant_t_k: f64,
    /// Exhaust gas temperature per cylinder, K, as the thermocouple reports it.
    pub egt_k: [f64; CYLINDERS],
    /// Cylinder head temperature per cylinder, K, as the sensor reports it.
    pub cht_k: [f64; CYLINDERS],
    /// Excess air ratio per cylinder.
    pub lambda: [f64; CYLINDERS],

    /// Electrical bus voltage, V. Not compared against the model, which has no
    /// alternator in it; carried so the health indices can score it.
    pub bus_v: f64,
    /// Broadband vibration, g RMS. Not compared, for the same reason.
    pub vib_rms_g: f64,
    /// Kurtosis of the vibration signal.
    pub vib_kurtosis: f64,
}

impl Measurement {
    /// Lay the measurement out in the order [`TABLE`] describes.
    #[must_use]
    pub fn vector(&self) -> [f64; CHANNELS] {
        let mut z = [0.0; CHANNELS];
        z[index::RPM] = self.rpm;
        z[index::MAP] = self.map_pa;
        z[index::MAT] = self.mat_k;
        z[index::MAF] = self.maf_kg_s;
        z[index::TURBO] = self.turbo_rpm;
        z[index::TORQUE] = self.torque_nm;
        z[index::FUEL_FLOW] = self.fuel_flow_kg_h;
        z[index::OIL_PRESSURE] = self.oil_p_pa;
        z[index::OIL_TEMPERATURE] = self.oil_t_k;
        z[index::COOLANT] = self.coolant_t_k;
        z[index::EGT..index::EGT + CYLINDERS].copy_from_slice(&self.egt_k);
        z[index::CHT..index::CHT + CYLINDERS].copy_from_slice(&self.cht_k);
        z[index::LAMBDA..index::LAMBDA + CYLINDERS].copy_from_slice(&self.lambda);
        z
    }

    /// True airspeed, m/s, which is what drives the radiator and the oil cooler.
    ///
    /// Indicated airspeed is what an airframe reports and mass flow is what cools,
    /// so the density ratio has to be undone. ISO 2533 sea level density.
    #[must_use]
    pub fn tas_m_s(&self) -> f64 {
        const RHO_SEA_LEVEL: f64 = 1.225;
        let rho = self.density();
        if rho <= 0.0 {
            return self.ias_m_s;
        }
        self.ias_m_s * (RHO_SEA_LEVEL / rho).sqrt()
    }

    /// Ambient air density, kg/m3.
    #[must_use]
    pub fn density(&self) -> f64 {
        const R_AIR: f64 = 287.05;
        if self.oat_k <= 0.0 {
            return 0.0;
        }
        self.p_amb_pa / (R_AIR * self.oat_k)
    }

    /// What the health indices need that the model does not describe.
    #[must_use]
    pub fn auxiliary(&self) -> crate::indices::Auxiliary {
        crate::indices::Auxiliary {
            bus_v: self.bus_v,
            vib_rms_g: self.vib_rms_g,
            vib_kurtosis: self.vib_kurtosis,
        }
    }

    /// Whether every field the filter needs is finite.
    ///
    /// A channel goes non-finite whenever its source falls silent, and one such
    /// value reaching the filter poisons the whole covariance rather than one row
    /// of it. The frame is dropped instead.
    #[must_use]
    pub fn is_usable(&self) -> bool {
        let scalars = [
            self.p_amb_pa,
            self.oat_k,
            self.ias_m_s,
            self.wastegate,
            self.injection_ms,
            self.rpm,
            self.map_pa,
            self.mat_k,
            self.maf_kg_s,
            self.turbo_rpm,
            self.torque_nm,
            self.fuel_flow_kg_h,
            self.oil_p_pa,
            self.oil_t_k,
            self.coolant_t_k,
        ];
        scalars.iter().all(|v| v.is_finite())
            && self
                .egt_k
                .iter()
                .chain(&self.cht_k)
                .chain(&self.lambda)
                .all(|v| v.is_finite())
            && self.rpm > 0.0
            && self.p_amb_pa > 0.0
            && self.oat_k > 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_and_the_indices_agree() {
        assert_eq!(TABLE.len(), CHANNELS);
        assert_eq!(TABLE[index::EGT + 2].name, "EGT 3");
        assert_eq!(TABLE[index::CHT].name, "CHT 1");
        assert_eq!(TABLE[index::LAMBDA + 3].name, "LAMBDA 4");
        assert_eq!(index::LAMBDA + CYLINDERS, CHANNELS);
    }

    /// The exhaust channel's standard deviation is what decides whether the fault
    /// this system exists to catch counts as evidence. At 13 K a 48 K departure is
    /// 3.7 standard deviations, which is the claim the diagnosis rests on.
    #[test]
    fn a_coked_cylinder_lands_where_the_exhaust_channel_says_it_should() {
        let sigma = TABLE[index::EGT + 2].sigma();
        let departure = 48.0 / sigma;
        assert!((3.5..3.9).contains(&departure), "{departure} sigma");
    }

    /// Every channel needs enough room that its own noise is not a diagnosis, and
    /// little enough that a real fault is not swallowed. The loosest is excess air
    /// ratio at four percent, which is a computed quantity rather than a measured
    /// one; nothing else exceeds two.
    #[test]
    fn every_channel_has_a_sane_ratio_of_noise_to_magnitude() {
        for c in &TABLE {
            let relative = c.sigma() / c.reference;
            assert!(relative > 1e-4, "{} is too certain: {relative}", c.name);
            assert!(relative < 0.05, "{} is too loose: {relative}", c.name);
        }
        for c in &TABLE[..index::LAMBDA] {
            assert!(c.sigma() / c.reference < 0.02, "{} is too loose", c.name);
        }
    }

    #[test]
    fn true_airspeed_exceeds_indicated_at_altitude() {
        let m = Measurement {
            t_s: 0.0,
            p_amb_pa: 42_070.0,
            oat_k: 242.15,
            ias_m_s: 40.1,
            ..blank()
        };
        // 40.1 m/s indicated at 22,400 ft on a standard day is 57.0 true, a
        // density ratio of 1.42. Any twin that fed indicated airspeed to the
        // radiator would under-cool the engine by a third at this altitude.
        assert!((m.tas_m_s() - 57.0).abs() < 0.5, "{}", m.tas_m_s());
        assert!(m.density() < 0.65, "{}", m.density());
    }

    #[test]
    fn a_silent_source_makes_the_frame_unusable() {
        let mut m = blank();
        m.rpm = 3720.0;
        m.p_amb_pa = 42_070.0;
        m.oat_k = 242.15;
        assert!(m.is_usable());
        m.egt_k[2] = f64::NAN;
        assert!(!m.is_usable());
    }

    fn blank() -> Measurement {
        Measurement {
            t_s: 0.0,
            p_amb_pa: 1.0,
            oat_k: 1.0,
            ias_m_s: 0.0,
            wastegate: 0.0,
            injection_ms: 0.0,
            rpm: 1.0,
            map_pa: 0.0,
            mat_k: 0.0,
            maf_kg_s: 0.0,
            turbo_rpm: 0.0,
            torque_nm: 0.0,
            fuel_flow_kg_h: 0.0,
            oil_p_pa: 0.0,
            oil_t_k: 0.0,
            coolant_t_k: 0.0,
            egt_k: [0.0; CYLINDERS],
            cht_k: [0.0; CYLINDERS],
            lambda: [0.0; CYLINDERS],
            bus_v: f64::NAN,
            vib_rms_g: f64::NAN,
            vib_kurtosis: f64::NAN,
        }
    }
}
