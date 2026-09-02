//! Degradation, injected as a parameter perturbation.
//!
//! An engine fault here is a change to a physical parameter of the engine, never
//! an offset added to a published signal. That distinction is the whole reason a
//! model-based health monitor is worth building: perturbing a parameter makes
//! every downstream channel move the way the physics says it must, including the
//! ones nobody thought to fake, and it is what lets a residual generator tell one
//! fault from another. Adding 47 K to an exhaust reading would reproduce the
//! headline number and nothing else about it.
//!
//! The two **sensor** faults are the deliberate exception, and they are the reason
//! the rule above is worth stating. A drifting or frozen instrument is a fault of
//! the signal and of nothing else: the engine is untouched, so every other channel
//! stays exactly where the physics puts it. That asymmetry is what a twin can see
//! and a threshold cannot. If EGT 3 climbs while fuel flow, torque and excess air
//! are all where the model says they should be, the cylinder is fine and the probe
//! is lying, and no amount of tightening the EGT limit will ever say so.
//!
//! Every progressive fault shares one growth shape, [`approach`]: an exponential
//! rise to a settled severity, reached exactly at the end of the ramp. Both
//! mechanisms it stands in for are measured to be asymptotic rather than linear,
//! and each carries its own note saying why.

use anyhow::{Result, ensure};
use clap::Args;
use dronecan_ice::{FaultCommand, FaultKind};
use engine_model::{CYLINDERS, EngineParams};

/// Shape constant for the growth curve. Three time constants reaches 95%.
const DECAY: f64 = 3.0;

// Named rather than repeated as literals: the command line and the bus are two
// entry points into one fault set, and a range that drifted between them would
// let one build an engine the other rejects. Both exclude zero separately.

/// Injector flow scale coking may settle at, as a fraction of nominal.
const COKING_SCALE: std::ops::Range<f64> = 0.0..1.0;

/// Fraction of firings a misfire may fail.
const MISFIRE_RATE: std::ops::RangeInclusive<f64> = 0.0..=1.0;

/// Radiator effectiveness fouling may settle at, as a fraction of clean.
const COOLING_SCALE: std::ops::Range<f64> = 0.0..1.0;

/// Severity at a given simulated time, 0 before the onset and 1 at the end of the
/// ramp.
///
/// Exponential rather than linear, and both faults that use it are measured to
/// behave this way for the same underlying reason: the deposit that causes the
/// fault also changes the conditions that grow it. Reaching exactly 1 at the end
/// of the ramp rather than asymptotically approaching it is a modelling
/// convenience, not a physical claim: it gives the fault a definite settled state
/// to diagnose against.
#[must_use]
fn approach(t_s: f64, onset_s: f64, ramp_s: f64) -> f64 {
    if t_s <= onset_s {
        return 0.0;
    }
    if ramp_s <= 0.0 {
        // A step change. Past the onset it is fully severe immediately.
        return 1.0;
    }
    let progress = ((t_s - onset_s) / ramp_s).clamp(0.0, 1.0);
    (1.0 - (-DECAY * progress).exp()) / (1.0 - (-DECAY).exp())
}

/// Progressive coking of one cylinder's injector nozzle.
///
/// Carbon deposits build in the nozzle holes and reduce the effective discharge
/// area, so that cylinder receives less fuel than commanded. It runs leaner, and
/// **its exhaust gets colder, not hotter**: a compression-ignition engine sits
/// far lean of any temperature peak, so heat release and exhaust temperature both
/// fall monotonically with fuel. The opposite intuition comes from spark ignition,
/// where the mixture is near stoichiometric and leaning moves toward the peak.
///
/// The engine also gives up a little torque, and nothing else about it changes,
/// which is what makes the signature narrow enough to diagnose.
#[derive(Clone, Copy, Debug)]
pub struct InjectorCoking {
    /// Affected cylinder, zero based.
    pub cylinder: usize,
    /// Simulated time at which deposits begin to matter, seconds.
    pub onset_s: f64,
    /// Time constant of the growth, seconds.
    pub ramp_s: f64,
    /// Injector flow scale the fault settles at, as a fraction of nominal.
    pub final_scale: f64,
}

impl InjectorCoking {
    /// Injector flow scale at a given simulated time, 1.0 being nominal.
    ///
    /// Deposits accumulate quickly at first and then slow: as the passage
    /// narrows, flow velocity through it rises and scours it, so growth is
    /// self-limiting rather than linear.
    #[must_use]
    pub fn scale_at(&self, t_s: f64) -> f64 {
        1.0 - (1.0 - self.final_scale) * approach(t_s, self.onset_s, self.ramp_s)
    }
}

/// Intermittent misfire in one cylinder.
///
/// Severity is the **fraction of that cylinder's firings that fail to ignite**,
/// which is the coordinate the experimental work uses and the only one a mean
/// value model can carry: averaged over the many cycles inside one telemetry
/// frame, a cylinder misfiring at rate `r` releases `1 - r` of its heat. Tamura
/// detects misfire from exactly this, the per-cylinder exhaust temperature falling
/// for seconds to tens of seconds against unchanged neighbours, and models the
/// observation as a first-order probe lag onto a firing-weighted mean of the normal
/// and misfiring temperatures; the worked case is 50% of firings failing at random
/// over ten seconds at 1,500 rpm, against a 15 s probe time constant.
///
/// That work is on **stationary lean-burn natural-gas engines**, so what transfers
/// is the parameterisation and the observable, not the combustion system. It is
/// cited because it is the study that measures what a slow per-cylinder exhaust
/// probe actually shows during intermittent misfire, which is the only channel this
/// model can offer.
///
/// What this model **cannot** show is the half-engine-order crankshaft speed
/// ripple that a crank-angle-resolved model produces and that most published
/// misfire detectors actually use. There are no individual cycles here to ripple.
/// The vibration channel carries the impulsiveness instead, in `sensors`, which is
/// a synthesised stand-in and says so.
///
/// Tamura, "Misfire detection of internal combustion engines using wave form of
/// exhaust gas temperature", Trans. JSME C 77(780), 2011, sec. 2.1 for the worked
/// case. The English companion is Tamura, Saito, Murata, Kokubu & Morimoto, Applied
/// Thermal Engineering 31(17-18), 2011, `10.1016/j.applthermaleng.2011.08.026`.
/// <https://doi.org/10.1299/kikaic.77.3094>
#[derive(Clone, Copy, Debug)]
pub struct Misfire {
    /// Affected cylinder, zero based.
    pub cylinder: usize,
    /// Simulated time at which misfiring begins, seconds.
    pub onset_s: f64,
    /// Time over which the misfire rate grows to its settled value, seconds.
    pub ramp_s: f64,
    /// Fraction of firings that fail once the fault has settled.
    pub final_rate: f64,
}

impl Misfire {
    /// Combustion efficiency of the affected cylinder, 1.0 being every firing good.
    #[must_use]
    pub fn efficiency_at(&self, t_s: f64) -> f64 {
        1.0 - self.final_rate * approach(t_s, self.onset_s, self.ramp_s)
    }
}

/// Progressive loss of radiator heat-rejection capability.
///
/// The mechanism is fouling: a deposit layer on the core acts as an insulating
/// barrier and the exchanger rejects less heat for the same temperature
/// difference. It is the one fault in this set that is **not** per cylinder, and
/// that is its signature: every cylinder head runs hotter together because they
/// all sit on one coolant loop, so a rise correlated across four channels is a
/// cooling fault and a rise on one is not.
///
/// Effectiveness loss is asymptotic, not linear. As the layer thickens the gas-side
/// surface runs hotter, which suppresses further deposition, so the loss plateaus.
/// The plateau measured on plain stainless diesel exhaust-cooler tubes is 16.7% of
/// effectiveness, and no anti-stick coating tested changed it. Those are exhaust
/// coolers rather than coolant radiators, which is a harsher duty on the gas side;
/// they are quoted here because they are the closest quantified diesel case, not
/// because a radiator was measured.
///
/// Storey, Sluder, Lance et al., "Exhaust gas recirculation cooler fouling in
/// diesel applications", Heat Exchanger Fouling and Cleaning XI, 2015, tab. 1.
/// <https://heatexchanger-fouling.com/wp-content/uploads/2021/09/11_Storey_F.pdf>
#[derive(Clone, Copy, Debug)]
pub struct CoolingDegradation {
    /// Simulated time at which fouling begins to matter, seconds.
    pub onset_s: f64,
    /// Time over which it reaches its plateau, seconds.
    pub ramp_s: f64,
    /// Radiator effectiveness the fault settles at, as a fraction of nominal.
    pub final_scale: f64,
}

impl CoolingDegradation {
    /// Radiator effectiveness scale at a given simulated time, 1.0 being clean.
    #[must_use]
    pub fn scale_at(&self, t_s: f64) -> f64 {
        1.0 - (1.0 - self.final_scale) * approach(t_s, self.onset_s, self.ramp_s)
    }
}

/// A ramp bias growing on one exhaust thermocouple.
///
/// The engine is untouched. This is the discriminator: a real fault moves a group
/// of channels the way the physics couples them, and a lying instrument moves one.
///
/// **The rate decides which physical failure this is, and the default is not probe
/// oxidation.** A type K thermocouple aged in place drifts far too slowly to matter
/// over a mission: conventional mineral-insulated probes average -2.3 K over 500 h
/// at 1200 C, about -0.005 K/h, and even a bare 0.5 mm element in the accelerated
/// stage of oxidation reaches only about 0.2 K/h. A rate above that is a fault in
/// the signal chain rather than in the junction: a loosening connector whose
/// contact resistance is climbing, a cracked sheath admitting exhaust to the
/// junction, or cold-junction compensation drifting with its own board temperature.
/// Those run at degrees per minute. The default sits between the two so a drift is
/// diagnosable inside a demonstration, and it is a signal-chain fault.
///
/// Tucker, Edler, Zuzek et al., "Thermoelectric stability of dual-wall and
/// conventional type K and N thermocouples", Meas. Sci. Technol. 33, 2022.
/// <https://doi.org/10.1088/1361-6501/ac57ee>
#[derive(Clone, Copy, Debug)]
pub struct SensorDrift {
    /// Cylinder whose exhaust probe is drifting, zero based.
    pub cylinder: usize,
    /// Simulated time at which the bias starts growing, seconds.
    pub onset_s: f64,
    /// Rate of the bias, K per hour. Signed: negative reads low.
    pub rate_k_per_h: f64,
}

impl SensorDrift {
    /// Bias to add to the affected probe at a given simulated time, K.
    #[must_use]
    pub fn bias_at(&self, t_s: f64) -> f64 {
        if t_s <= self.onset_s {
            return 0.0;
        }
        self.rate_k_per_h * (t_s - self.onset_s) / 3600.0
    }
}

/// One exhaust probe that stops updating and holds its last value.
///
/// The signature is **zero variance**, not a wrong number, and it is why the frozen
/// value is captured after the measurement noise rather than before: a channel that
/// stops moving at all is detectable long before its held value has drifted far
/// enough from the truth to trip anything. A held reading is also the most
/// dangerous failure on this list, because it looks healthy from every direction
/// until the engine moves away from wherever it was when the channel died.
#[derive(Clone, Copy, Debug)]
pub struct SensorFreeze {
    /// Cylinder whose exhaust probe stops updating, zero based.
    pub cylinder: usize,
    /// Simulated time at which it stops, seconds.
    pub onset_s: f64,
}

/// Everything that can be wrong with the engine on one run.
#[derive(Clone, Copy, Debug, Default)]
pub struct Faults {
    /// Injector coking, if injected.
    pub injector: Option<InjectorCoking>,
    /// Misfire, if injected.
    pub misfire: Option<Misfire>,
    /// Radiator fouling, if injected.
    pub cooling: Option<CoolingDegradation>,
    /// Exhaust probe drift, if injected.
    pub drift: Option<SensorDrift>,
    /// Exhaust probe freeze, if injected.
    pub freeze: Option<SensorFreeze>,
}

impl Faults {
    /// Write the engine-side faults into `params` for this instant.
    ///
    /// Called before the step rather than after it, so the engine integrates with
    /// the degraded parameter rather than one step behind it.
    ///
    /// Every severity multiplies the value in `base`, which must be the parameter
    /// set as loaded, never `params` itself. Scaling a value in place would
    /// compound it once per call and the fault would run away over a mission
    /// instead of settling. Going through `base` also means a parameter file
    /// describing an already worn engine composes with an injected fault rather
    /// than being overwritten by it.
    pub fn apply(&self, params: &mut EngineParams, base: &EngineParams, t_s: f64) {
        // Restored from `base` first, every call, so this is a pure function of
        // the fault set rather than an accumulation of past ones. Two things
        // depend on it and neither is hypothetical: a fault cleared over the bus
        // has to give the engine back, and a fault moved to another cylinder has
        // to leave the one it came from healthy. Writing only the faulted
        // parameter left the last degraded value in place forever, which on a
        // cleared engine looked exactly like a command that never arrived.
        params.cylinder.injector_scale = base.cylinder.injector_scale;
        params.cylinder.combustion_efficiency = base.cylinder.combustion_efficiency;
        params.cooling.radiator_effectiveness = base.cooling.radiator_effectiveness;

        if let Some(f) = self.injector {
            params.cylinder.injector_scale[f.cylinder] =
                base.cylinder.injector_scale[f.cylinder] * f.scale_at(t_s);
        }
        if let Some(f) = self.misfire {
            params.cylinder.combustion_efficiency[f.cylinder] =
                base.cylinder.combustion_efficiency[f.cylinder] * f.efficiency_at(t_s);
        }
        if let Some(f) = self.cooling {
            params.cooling.radiator_effectiveness =
                base.cooling.radiator_effectiveness * f.scale_at(t_s);
        }
    }

    /// Apply a fault commanded over the bus, starting from now.
    ///
    /// The command carries no onset because a commanded fault begins when it
    /// arrives: `t_s` is the simulated instant the frame was decoded, and every
    /// ramp runs from there. That is the difference from the command line
    /// arguments, which schedule a fault at a time chosen before the run starts.
    ///
    /// `&'static str` and not a `thiserror` enum: the one caller logs the reason
    /// and has no other move to make, so variants would serve nothing.
    ///
    /// Returns why a command was rejected, and applies nothing when it is. An
    /// unknown kind is one, so an older simulator leaves the engine alone rather
    /// than guessing. **The bus is not trusted**: a severity is a float16 anything
    /// on the interface can write, and a non-finite one propagates through the
    /// integration until every published channel is NaN.
    ///
    /// A commanded fault **replaces** the one of its kind rather than composing
    /// with it. Two coking faults on one cylinder would otherwise ramp from two
    /// different onsets and the settled value would depend on the order the
    /// buttons were pressed.
    pub fn command(&mut self, command: FaultCommand, t_s: f64) -> Result<(), &'static str> {
        let Some(kind) = FaultKind::from_u8(command.kind) else {
            return Err("unknown fault kind");
        };
        // One based on the wire, zero based here, and clamped rather than
        // rejected: a cylinder index out of range is a ground station bug, and
        // dropping the command silently would look like the bus lost it.
        let cylinder = usize::from(command.cylinder.clamp(1, CYLINDERS as u8) - 1);
        let severity = f64::from(command.severity);
        let ramp_s = f64::from(command.ramp_s);
        if !ramp_s.is_finite() || ramp_s < 0.0 {
            return Err("ramp is not a duration in seconds");
        }
        if !severity.is_finite() {
            return Err("severity is not finite");
        }
        match kind {
            FaultKind::InjectorCoking if !COKING_SCALE.contains(&severity) => {
                return Err("coking severity is not a fraction of nominal injector flow below 1.0");
            }
            FaultKind::Misfire if !MISFIRE_RATE.contains(&severity) || severity <= 0.0 => {
                return Err("misfire severity is not a fraction of firings above 0.0");
            }
            FaultKind::CoolingDegradation if !COOLING_SCALE.contains(&severity) => {
                return Err(
                    "cooling severity is not a fraction of nominal radiator effectiveness below 1.0",
                );
            }
            FaultKind::SensorDrift if severity == 0.0 => {
                return Err("a drift of 0 K/h injects nothing");
            }
            _ => {}
        }

        match kind {
            FaultKind::Clear => *self = Self::default(),
            FaultKind::InjectorCoking => {
                self.injector = Some(InjectorCoking {
                    cylinder,
                    onset_s: t_s,
                    ramp_s,
                    final_scale: severity,
                });
            }
            FaultKind::Misfire => {
                self.misfire = Some(Misfire {
                    cylinder,
                    onset_s: t_s,
                    ramp_s,
                    final_rate: severity,
                });
            }
            FaultKind::SensorDrift => {
                self.drift = Some(SensorDrift {
                    cylinder,
                    onset_s: t_s,
                    rate_k_per_h: severity,
                });
            }
            FaultKind::SensorFreeze => {
                self.freeze = Some(SensorFreeze {
                    cylinder,
                    onset_s: t_s,
                });
            }
            FaultKind::CoolingDegradation => {
                self.cooling = Some(CoolingDegradation {
                    onset_s: t_s,
                    ramp_s,
                    final_scale: severity,
                });
            }
        }
        Ok(())
    }

    /// Corrupt the exhaust readings the way a failing instrument would.
    ///
    /// Call this **after** the probe lag and after the measurement noise: both
    /// faults are in the instrument and not in the gas. That ordering is what
    /// gives a frozen probe its signature, since it then holds one noisy sample
    /// forever and its variance goes to zero, which is detectable long before the
    /// held value has drifted far enough from the truth to cross any limit.
    ///
    /// `held` is the caller's storage for the frozen sample and must persist
    /// across calls. A frozen channel is a state rather than a function of time,
    /// so this cannot be derived from `t_s` alone.
    pub fn corrupt_exhaust(&self, egt_k: &mut [f64; CYLINDERS], held: &mut Option<f64>, t_s: f64) {
        if let Some(d) = self.drift {
            egt_k[d.cylinder] += d.bias_at(t_s);
        }
        if let Some(f) = self.freeze
            && t_s >= f.onset_s
        {
            egt_k[f.cylinder] = *held.get_or_insert(egt_k[f.cylinder]);
        }
    }
}

#[cfg(test)]
mod command_tests {
    use super::*;
    use dronecan_ice::FaultKind;

    fn command(kind: FaultKind, cylinder: u8, severity: f32, ramp_s: f32) -> FaultCommand {
        FaultCommand {
            sequence: 0,
            kind: kind as u8,
            cylinder,
            severity,
            ramp_s,
        }
    }

    /// The one a live run found: clearing has to give the engine back.
    ///
    /// `apply` used to write only the faulted parameter, so a cleared fault left
    /// the last degraded value in the plant for the rest of the mission and the
    /// clear looked like a command that never arrived.
    #[test]
    fn clearing_restores_the_engine() {
        let base = engine_model::engines::ae330();
        let mut params = base.clone();
        let mut faults = Faults::default();

        assert!(
            faults
                .command(command(FaultKind::InjectorCoking, 3, 0.72, 0.0), 0.0)
                .is_ok()
        );
        faults.apply(&mut params, &base, 600.0);
        let coked = params.cylinder.injector_scale[2];
        assert!(
            coked < base.cylinder.injector_scale[2] * 0.8,
            "fault applied"
        );

        assert!(
            faults
                .command(command(FaultKind::Clear, 0, 0.0, 0.0), 600.0)
                .is_ok()
        );
        faults.apply(&mut params, &base, 900.0);
        assert_eq!(
            params.cylinder.injector_scale[2],
            base.cylinder.injector_scale[2]
        );
    }

    /// Moving a fault to another cylinder has to leave the first one healthy.
    #[test]
    fn a_moved_fault_leaves_its_old_cylinder_alone() {
        let base = engine_model::engines::ae330();
        let mut params = base.clone();
        let mut faults = Faults::default();

        assert!(
            faults
                .command(command(FaultKind::InjectorCoking, 3, 0.72, 0.0), 0.0)
                .is_ok()
        );
        faults.apply(&mut params, &base, 600.0);
        assert!(
            faults
                .command(command(FaultKind::InjectorCoking, 1, 0.72, 0.0), 600.0)
                .is_ok()
        );
        faults.apply(&mut params, &base, 1200.0);

        assert_eq!(
            params.cylinder.injector_scale[2],
            base.cylinder.injector_scale[2]
        );
        assert!(params.cylinder.injector_scale[0] < base.cylinder.injector_scale[0] * 0.8);
    }

    #[test]
    fn an_unknown_kind_changes_nothing() {
        let mut faults = Faults::default();
        let unknown = FaultCommand {
            sequence: 0,
            kind: 200,
            cylinder: 1,
            severity: 1.0,
            ramp_s: 0.0,
        };
        assert!(faults.command(unknown, 0.0).is_err());
        assert!(faults.injector.is_none() && faults.misfire.is_none());
    }

    /// Anything on the interface can write a float16, and a non-finite one would
    /// otherwise reach the integrator and turn every published channel into NaN.
    #[test]
    fn a_severity_the_bus_cannot_mean_is_refused() {
        let refused = [
            command(FaultKind::InjectorCoking, 3, f32::NAN, 0.0),
            command(FaultKind::InjectorCoking, 3, -0.5, 0.0),
            // 1.0 is nominal flow, which injects nothing at all.
            command(FaultKind::InjectorCoking, 3, 1.0, 0.0),
            command(FaultKind::Misfire, 3, 0.0, 0.0),
            command(FaultKind::Misfire, 3, 1.5, 0.0),
            command(FaultKind::CoolingDegradation, 0, f32::INFINITY, 0.0),
            command(FaultKind::SensorDrift, 3, 0.0, 0.0),
            command(FaultKind::InjectorCoking, 3, 0.72, f32::NAN),
            command(FaultKind::InjectorCoking, 3, 0.72, -1.0),
        ];
        for c in refused {
            let mut faults = Faults::default();
            assert!(faults.command(c, 0.0).is_err(), "{c:?} was accepted");
            assert!(
                faults.injector.is_none()
                    && faults.misfire.is_none()
                    && faults.cooling.is_none()
                    && faults.drift.is_none(),
                "{c:?} changed the engine"
            );
        }
    }
}

/// Fault injection as it appears on the command line.
///
/// A separate `Args` group rather than fields on the main parser: this is half the
/// surface of the binary and all of it is one subject, and the fault console the UI
/// will grow at D9 sends exactly these.
#[derive(Args, Clone, Copy, Debug)]
pub struct FaultArgs {
    /// Cylinder to coke the injector on, 1 to 4. No injector fault if unset.
    #[arg(long, value_parser = clap::value_parser!(u8).range(1..=4))]
    pub fault_cylinder: Option<u8>,

    /// Simulated seconds before the injector fault begins.
    #[arg(long, default_value_t = 90.0)]
    pub fault_onset: f64,

    /// Simulated seconds the injector fault takes to reach its settled severity.
    #[arg(long, default_value_t = 240.0)]
    pub fault_ramp: f64,

    /// Injector flow scale the fault settles at, as a fraction of nominal.
    #[arg(long, default_value_t = 0.84)]
    pub fault_scale: f64,

    /// Cylinder to misfire, 1 to 4. No misfire if unset.
    #[arg(long, value_parser = clap::value_parser!(u8).range(1..=4))]
    pub misfire_cylinder: Option<u8>,

    /// Simulated seconds before misfiring begins.
    #[arg(long, default_value_t = 90.0)]
    pub misfire_onset: f64,

    /// Simulated seconds over which the misfire rate grows.
    #[arg(long, default_value_t = 120.0)]
    pub misfire_ramp: f64,

    /// Fraction of that cylinder's firings that fail once settled, 0 to 1.
    #[arg(long, default_value_t = 0.20)]
    pub misfire_rate: f64,

    /// Foul the radiator. No cooling fault if unset.
    #[arg(long)]
    pub cooling_fault: bool,

    /// Simulated seconds before fouling begins to matter.
    #[arg(long, default_value_t = 90.0)]
    pub cooling_onset: f64,

    /// Simulated seconds over which fouling reaches its plateau.
    #[arg(long, default_value_t = 600.0)]
    pub cooling_ramp: f64,

    /// Radiator effectiveness the fault settles at, as a fraction of nominal.
    #[arg(long, default_value_t = 0.83)]
    pub cooling_scale: f64,

    /// Cylinder whose exhaust probe drifts, 1 to 4. No drift if unset.
    #[arg(long, value_parser = clap::value_parser!(u8).range(1..=4))]
    pub drift_cylinder: Option<u8>,

    /// Simulated seconds before the bias starts growing.
    #[arg(long, default_value_t = 90.0)]
    pub drift_onset: f64,

    /// Bias growth rate, K per hour. Negative reads low. See `SensorDrift`.
    #[arg(long, default_value_t = 12.0)]
    pub drift_rate: f64,

    /// Cylinder whose exhaust probe freezes, 1 to 4. No freeze if unset.
    #[arg(long, value_parser = clap::value_parser!(u8).range(1..=4))]
    pub freeze_cylinder: Option<u8>,

    /// Simulated seconds before the probe stops updating.
    #[arg(long, default_value_t = 90.0)]
    pub freeze_onset: f64,
}

/// Reject a time that would propagate through the integrator or silently disable a
/// fault the command line says was injected.
fn check_timing(onset_s: f64, ramp_s: f64, what: &str) -> Result<()> {
    ensure!(
        onset_s.is_finite() && onset_s >= 0.0,
        "--{what}-onset is a simulated time in seconds, so it must be finite and not negative, got {onset_s}"
    );
    ensure!(
        ramp_s.is_finite() && ramp_s >= 0.0,
        "--{what}-ramp is a duration in seconds, so it must be finite and not negative, got {ramp_s}. Use 0 for a step change."
    );
    Ok(())
}

impl FaultArgs {
    /// Validate and build the fault set.
    ///
    /// The checks matter because every one of these failures is **silent**: clamped
    /// progress or a severity of zero publishes a healthy engine for a run whose
    /// command line says a fault was injected, and a non-finite value propagates
    /// through the state integration until every channel on the bus is NaN. An
    /// argument for a fault that was not injected is inert and is not checked, or a
    /// healthy run would fail for no reason.
    pub fn build(&self) -> Result<Faults> {
        let mut faults = Faults::default();

        if let Some(c) = self.fault_cylinder {
            check_timing(self.fault_onset, self.fault_ramp, "fault")?;
            ensure!(
                self.fault_scale.is_finite() && COKING_SCALE.contains(&self.fault_scale),
                "--fault-scale is the fraction of nominal injector flow the fault settles at, so it must be at least 0.0 and below 1.0 to remove any fuel at all, got {}",
                self.fault_scale
            );
            faults.injector = Some(InjectorCoking {
                cylinder: usize::from(c - 1),
                onset_s: self.fault_onset,
                ramp_s: self.fault_ramp,
                final_scale: self.fault_scale,
            });
        }

        if let Some(c) = self.misfire_cylinder {
            check_timing(self.misfire_onset, self.misfire_ramp, "misfire")?;
            ensure!(
                self.misfire_rate.is_finite() && MISFIRE_RATE.contains(&self.misfire_rate),
                "--misfire-rate is the fraction of firings that fail, so it must be between 0.0 and 1.0, got {}",
                self.misfire_rate
            );
            ensure!(
                self.misfire_rate > 0.0,
                "--misfire-rate 0 injects nothing. Omit --misfire-cylinder for a healthy run."
            );
            faults.misfire = Some(Misfire {
                cylinder: usize::from(c - 1),
                onset_s: self.misfire_onset,
                ramp_s: self.misfire_ramp,
                final_rate: self.misfire_rate,
            });
        }

        if self.cooling_fault {
            check_timing(self.cooling_onset, self.cooling_ramp, "cooling")?;
            ensure!(
                self.cooling_scale.is_finite() && COOLING_SCALE.contains(&self.cooling_scale),
                "--cooling-scale is the fraction of nominal radiator effectiveness the fault settles at, so it must be at least 0.0 and below 1.0 to degrade cooling at all, got {}",
                self.cooling_scale
            );
            faults.cooling = Some(CoolingDegradation {
                onset_s: self.cooling_onset,
                ramp_s: self.cooling_ramp,
                final_scale: self.cooling_scale,
            });
        }

        if let Some(c) = self.drift_cylinder {
            check_timing(self.drift_onset, 0.0, "drift")?;
            ensure!(
                self.drift_rate.is_finite() && self.drift_rate != 0.0,
                "--drift-rate is a bias growth in K per hour, so it must be finite and not zero, got {}",
                self.drift_rate
            );
            faults.drift = Some(SensorDrift {
                cylinder: usize::from(c - 1),
                onset_s: self.drift_onset,
                rate_k_per_h: self.drift_rate,
            });
        }

        if let Some(c) = self.freeze_cylinder {
            check_timing(self.freeze_onset, 0.0, "freeze")?;
            faults.freeze = Some(SensorFreeze {
                cylinder: usize::from(c - 1),
                onset_s: self.freeze_onset,
            });
        }

        if let (Some(d), Some(f)) = (faults.drift, faults.freeze) {
            ensure!(
                d.cylinder != f.cylinder,
                "--drift-cylinder and --freeze-cylinder are both {}: a frozen probe cannot also drift, and the freeze would silently win.",
                d.cylinder + 1
            );
        }

        Ok(faults)
    }

    /// A one-line summary for the startup log, naming only what was injected.
    #[must_use]
    pub fn summary(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        if let Some(c) = self.fault_cylinder {
            parts.push(format!("injector-{c} coking to {}", self.fault_scale));
        }
        if let Some(c) = self.misfire_cylinder {
            parts.push(format!("misfire cyl {c} at {}", self.misfire_rate));
        }
        if self.cooling_fault {
            parts.push(format!("radiator fouling to {}", self.cooling_scale));
        }
        if let Some(c) = self.drift_cylinder {
            parts.push(format!("EGT-{c} drift {} K/h", self.drift_rate));
        }
        if let Some(c) = self.freeze_cylinder {
            parts.push(format!("EGT-{c} frozen"));
        }
        if parts.is_empty() {
            "healthy".into()
        } else {
            parts.join(", ")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[derive(Parser)]
    struct Harness {
        #[command(flatten)]
        faults: FaultArgs,
    }

    /// `try_parse_from`, not `parse_from`: the latter exits the process on a parse
    /// error, which takes the whole test binary with it.
    fn args(extra: &[&str]) -> FaultArgs {
        let mut argv = vec!["dragonfly-sim"];
        argv.extend_from_slice(extra);
        Harness::try_parse_from(argv)
            .expect("arguments should parse")
            .faults
    }

    fn coking() -> InjectorCoking {
        InjectorCoking {
            cylinder: 2,
            onset_s: 100.0,
            ramp_s: 200.0,
            final_scale: 0.84,
        }
    }

    #[test]
    fn nominal_before_onset() {
        assert_eq!(coking().scale_at(0.0), 1.0);
        assert_eq!(coking().scale_at(100.0), 1.0);
    }

    /// The "before" is what proves a twin was ever locked, so the fault must start
    /// from exactly nominal rather than from a small standing offset.
    #[test]
    fn reaches_its_final_scale_and_holds() {
        let f = coking();
        assert!((f.scale_at(300.0) - 0.84).abs() < 1e-12);
        assert!((f.scale_at(1e6) - 0.84).abs() < 1e-12);
    }

    #[test]
    fn growth_decelerates() {
        let f = coking();
        let first = 1.0 - f.scale_at(150.0);
        let second = (1.0 - f.scale_at(200.0)) - first;
        assert!(first > second, "{first} then {second}");
    }

    /// A zero ramp is a step change and is allowed. It must not read as "never
    /// started", which is what a naive division would give.
    #[test]
    fn a_zero_ramp_steps_straight_to_the_settled_severity() {
        assert_eq!(approach(10.0, 20.0, 0.0), 0.0);
        assert_eq!(approach(30.0, 20.0, 0.0), 1.0);
    }

    #[test]
    fn misfire_removes_its_share_of_the_heat_release() {
        let m = Misfire {
            cylinder: 0,
            onset_s: 10.0,
            ramp_s: 100.0,
            final_rate: 0.20,
        };
        assert_eq!(m.efficiency_at(0.0), 1.0);
        assert!((m.efficiency_at(110.0) - 0.80).abs() < 1e-12);
    }

    #[test]
    fn drift_grows_linearly_from_its_onset_and_not_before() {
        let d = SensorDrift {
            cylinder: 2,
            onset_s: 60.0,
            rate_k_per_h: 12.0,
        };
        assert_eq!(d.bias_at(0.0), 0.0);
        assert_eq!(d.bias_at(60.0), 0.0);
        assert!((d.bias_at(60.0 + 3600.0) - 12.0).abs() < 1e-12);
        assert!((d.bias_at(60.0 + 1800.0) - 6.0).abs() < 1e-12);
    }

    /// Only the parameters a fault names may move. A cooling fault that also
    /// nudged an injector would make every diagnosis downstream ambiguous, and the
    /// test that catches it has to look at the parameter set, not at the channels.
    #[test]
    fn each_fault_touches_only_its_own_parameter() {
        let base = engine_model::engines::ae330();

        let mut p = base.clone();
        args(&["--misfire-cylinder", "3"])
            .build()
            .unwrap()
            .apply(&mut p, &base, 1e5);
        assert!(p.cylinder.combustion_efficiency[2] < 0.99);
        assert_eq!(p.cylinder.injector_scale, base.cylinder.injector_scale);
        assert_eq!(
            p.cooling.radiator_effectiveness,
            base.cooling.radiator_effectiveness
        );

        let mut p = base.clone();
        args(&["--fault-cylinder", "3"])
            .build()
            .unwrap()
            .apply(&mut p, &base, 1e5);
        assert!(p.cylinder.injector_scale[2] < 0.99);
        assert_eq!(
            p.cylinder.combustion_efficiency,
            base.cylinder.combustion_efficiency
        );

        let mut p = base.clone();
        args(&["--cooling-fault"])
            .build()
            .unwrap()
            .apply(&mut p, &base, 1e5);
        assert!(p.cooling.radiator_effectiveness < base.cooling.radiator_effectiveness);
        assert_eq!(p.cylinder.injector_scale, base.cylinder.injector_scale);
        assert_eq!(
            p.cylinder.combustion_efficiency,
            base.cylinder.combustion_efficiency
        );
    }

    /// A sensor fault is a fault of the signal. If it reaches the engine parameters
    /// at all, the discriminator this whole feature exists for is broken.
    #[test]
    fn a_sensor_fault_never_reaches_the_engine() {
        let base = engine_model::engines::ae330();
        let mut p = base.clone();
        args(&["--drift-cylinder", "3", "--freeze-cylinder", "1"])
            .build()
            .unwrap()
            .apply(&mut p, &base, 1e5);
        assert_eq!(p.cylinder.injector_scale, base.cylinder.injector_scale);
        assert_eq!(
            p.cylinder.combustion_efficiency,
            base.cylinder.combustion_efficiency
        );
        assert_eq!(
            p.cooling.radiator_effectiveness,
            base.cooling.radiator_effectiveness
        );
    }

    /// The failures these guard are silent, not loud: the run looks healthy while
    /// the command line says otherwise.
    #[test]
    fn severities_that_would_disable_their_fault_are_rejected() {
        assert!(
            args(&["--fault-cylinder", "3", "--fault-ramp=-1"])
                .build()
                .is_err()
        );
        assert!(
            args(&["--fault-cylinder", "3", "--fault-scale", "1.2"])
                .build()
                .is_err()
        );
        assert!(
            args(&["--fault-cylinder", "3", "--fault-scale", "nan"])
                .build()
                .is_err()
        );
        assert!(
            args(&["--misfire-cylinder", "3", "--misfire-rate", "0"])
                .build()
                .is_err()
        );
        assert!(
            args(&["--misfire-cylinder", "3", "--misfire-rate", "1.5"])
                .build()
                .is_err()
        );
        assert!(
            args(&["--cooling-fault", "--cooling-scale", "1.0"])
                .build()
                .is_err()
        );
        assert!(
            args(&["--drift-cylinder", "3", "--drift-rate", "0"])
                .build()
                .is_err()
        );
    }

    /// The extreme of the same fault, not a different one. `--fault-scale 0` is a
    /// totally blocked injector and `--misfire-rate 1` is a dead cylinder; the
    /// model already answers for both, returning infinite lambda on zero burned
    /// fuel and reading a non-finite lambda as zero equivalence ratio.
    #[test]
    fn the_extremes_of_each_fault_are_allowed() {
        assert!(
            args(&["--fault-cylinder", "3", "--fault-scale", "0"])
                .build()
                .is_ok()
        );
        assert!(
            args(&["--misfire-cylinder", "3", "--misfire-rate", "1"])
                .build()
                .is_ok()
        );
    }

    /// Two instrument faults on one probe would compose in whichever order the code
    /// happens to apply them, and the freeze would hide the drift entirely.
    #[test]
    fn a_probe_cannot_be_frozen_and_drifting_at_once() {
        assert!(
            args(&["--drift-cylinder", "3", "--freeze-cylinder", "3"])
                .build()
                .is_err()
        );
        assert!(
            args(&["--drift-cylinder", "3", "--freeze-cylinder", "1"])
                .build()
                .is_ok()
        );
    }

    /// Without a cylinder there is no fault, so the severities are inert and
    /// rejecting them would fail a healthy run for no reason.
    #[test]
    fn fault_arguments_are_inert_on_a_healthy_run() {
        let healthy = args(&["--fault-scale", "9", "--misfire-rate", "9"]);
        let built = healthy
            .build()
            .expect("inert severities must not fail a run");
        assert!(built.injector.is_none() && built.misfire.is_none());
        assert_eq!(healthy.summary(), "healthy");
    }

    #[test]
    fn defaults_describe_the_demonstration_fault() {
        let demo = args(&["--fault-cylinder", "3"]);
        let built = demo.build().expect("the demo fault must build");
        assert!(built.injector.is_some());
        assert!(demo.summary().contains("injector-3"));
    }

    #[test]
    fn faults_compose_without_overwriting_one_another() {
        let both = args(&["--fault-cylinder", "1", "--misfire-cylinder", "4"])
            .build()
            .unwrap();
        let base = engine_model::engines::ae330();
        let mut p = base.clone();
        both.apply(&mut p, &base, 1e5);
        assert!(p.cylinder.injector_scale[0] < 0.99);
        assert!(p.cylinder.combustion_efficiency[3] < 0.99);
        for i in 1..CYLINDERS {
            assert_eq!(p.cylinder.injector_scale[i], 1.0, "injector {i}");
        }
    }
}
