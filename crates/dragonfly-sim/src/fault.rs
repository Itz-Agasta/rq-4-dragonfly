//! Degradation, injected as a parameter perturbation.
//!
//! A fault here is a change to a physical parameter of the engine, never an
//! offset added to a published signal. That distinction is the whole reason a
//! model-based health monitor is worth building: perturbing a parameter makes
//! every downstream channel move the way the physics says it must, including the
//! ones nobody thought to fake, and it is what lets a residual generator tell one
//! fault from another. Adding 47 K to an exhaust reading would reproduce the
//! headline number and nothing else about it.

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

/// Shape constant for the growth curve. Three time constants reaches 95%.
const DECAY: f64 = 3.0;

impl InjectorCoking {
    /// Injector flow scale at a given simulated time, 1.0 being nominal.
    ///
    /// Deposits accumulate quickly at first and then slow: as the passage
    /// narrows, flow velocity through it rises and scours it, so growth is
    /// self-limiting rather than linear. Modelled as an exponential approach to
    /// the final scale, reaching it exactly at the end of the ramp so the fault
    /// has a definite settled state to diagnose against.
    #[must_use]
    pub fn scale_at(&self, t_s: f64) -> f64 {
        if t_s <= self.onset_s {
            return 1.0;
        }
        let progress = ((t_s - self.onset_s) / self.ramp_s).clamp(0.0, 1.0);
        let growth = (1.0 - (-DECAY * progress).exp()) / (1.0 - (-DECAY).exp());
        1.0 - (1.0 - self.final_scale) * growth
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fault() -> InjectorCoking {
        InjectorCoking {
            cylinder: 2,
            onset_s: 100.0,
            ramp_s: 200.0,
            final_scale: 0.84,
        }
    }

    #[test]
    fn nominal_before_onset() {
        assert_eq!(fault().scale_at(0.0), 1.0);
        assert_eq!(fault().scale_at(100.0), 1.0);
    }

    /// The "before" is what proves a twin was ever locked, so the fault must
    /// start from exactly nominal rather than from a small standing offset.
    #[test]
    fn reaches_its_final_scale_and_holds() {
        let f = fault();
        assert!((f.scale_at(300.0) - 0.84).abs() < 1e-12);
        assert!((f.scale_at(1e6) - 0.84).abs() < 1e-12);
    }

    #[test]
    fn growth_decelerates() {
        let f = fault();
        let first = 1.0 - f.scale_at(150.0);
        let second = (1.0 - f.scale_at(200.0)) - first;
        assert!(first > second, "{first} then {second}");
    }
}
