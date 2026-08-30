//! The engine and the load it drives.
//!
//! `engine-model` is a plant: it takes an absorbed torque and reports what the
//! crankshaft does about it. Something has to supply that torque, and on this
//! airframe it is a constant-speed propeller with a governor. The published
//! rating point is quoted at a propeller torque, so the propeller is part of what
//! makes the model reproduce it.
//!
//! Two controllers close here, both outside the engine model on purpose. The
//! boost controller drives the wastegate, which the engine takes as an input so
//! that a twin can replay a recorded wastegate position instead of re-deriving
//! it. The propeller governor drives blade pitch. Neither belongs to the plant.

use engine_model::{
    EngineParams, Inputs, Outputs, State, control::BoostController, integrator, step,
};

use crate::mission::Condition;

/// Propeller diameter, m. **estimated** from the airframe class; the published
/// data gives torque and speed but not the propeller.
const DIAMETER_M: f64 = 1.8;

/// Torque coefficient at the fine pitch stop, dimensionless. **estimated** to
/// bracket the value the published rating point implies, about 0.016 at sea level.
const CQ_MIN: f64 = 0.004;
/// Torque coefficient at the coarse pitch stop.
const CQ_MAX: f64 = 0.060;
/// Torque coefficient the governor trims around.
const CQ_TRIM_CENTRE: f64 = 0.016;

/// Governor proportional gain, torque coefficient per rpm of error. **estimated**:
/// fast enough to hold speed through a fuelling step, slow enough not to fight
/// the manifold.
const GOVERNOR_KP: f64 = 2.0e-6;
/// Governor integral gain, torque coefficient per rpm-second of error.
const GOVERNOR_KI: f64 = 4.0e-6;

/// A constant-speed propeller and its governor.
///
/// Absorbed torque follows the standard propeller law `Q = c_q rho n^2 D^5` with
/// `n` in revolutions per second at the propeller. The governor trims `c_q`,
/// which is what blade pitch physically does, and saturates at the pitch stops.
/// Beyond the stops the engine runs away or bogs down, both of which are real.
#[derive(Clone, Copy, Debug)]
pub struct Propeller {
    torque_coefficient: f64,
    integral: f64,
}

impl Default for Propeller {
    fn default() -> Self {
        Self {
            torque_coefficient: CQ_TRIM_CENTRE,
            integral: 0.0,
        }
    }
}

impl Propeller {
    /// Torque absorbed, referred to the crankshaft, N.m.
    ///
    /// Power is conserved across the reduction gearbox, so crankshaft torque is
    /// propeller torque divided by the ratio rather than multiplied by it.
    #[must_use]
    pub fn absorbed_torque(&self, p: &EngineParams, rpm: f64, rho: f64) -> f64 {
        let n_prop = rpm / p.geometry.gearbox_ratio / 60.0;
        let q_prop = self.torque_coefficient * rho * n_prop * n_prop * DIAMETER_M.powi(5);
        q_prop / p.geometry.gearbox_ratio
    }

    /// Advance the governor one step towards the commanded speed.
    pub fn govern(&mut self, rpm: f64, rpm_cmd: f64, dt: f64) {
        let error = rpm_cmd - rpm;
        self.integral += error * dt;
        // Clamped in the integrator rather than only at the output, so a
        // saturated pitch stop cannot charge it up during a climb and then
        // overshoot on the way back down.
        let integral_limit = CQ_MAX / GOVERNOR_KI;
        self.integral = self.integral.clamp(-integral_limit, integral_limit);
        let trim = GOVERNOR_KP.mul_add(-error, -(GOVERNOR_KI * self.integral));
        self.torque_coefficient = (CQ_TRIM_CENTRE + trim).clamp(CQ_MIN, CQ_MAX);
    }
}

/// The engine, its load, and the controllers that close around both.
#[derive(Debug)]
pub struct Plant {
    /// Parameter set the engine runs on.
    pub params: EngineParams,
    /// Current engine state.
    pub state: State,
    /// Fuel burnt since the engine started, m3.
    pub fuel_burnt_m3: f64,
    boost: BoostController,
    propeller: Propeller,
    wastegate: f64,
}

impl Plant {
    /// Start warm at the profile's initial speed, which is what a health monitor
    /// almost always sees. A cold start takes minutes of simulated time to settle.
    #[must_use]
    pub fn new(params: EngineParams, condition: &Condition) -> Self {
        Self {
            state: State::at_rest(condition.p_amb, condition.rpm_cmd),
            params,
            fuel_burnt_m3: 0.0,
            boost: BoostController::new(),
            propeller: Propeller::default(),
            wastegate: 1.0,
        }
    }

    /// Wastegate position the boost controller last commanded, 0 shut to 1 open.
    #[must_use]
    pub fn wastegate(&self) -> f64 {
        self.wastegate
    }

    /// Integrate forward by `duration` seconds and report the final operating point.
    ///
    /// Sub-steps at the model's fixed 200 Hz rather than taking one long step:
    /// the turbocharger shaft is the stiff state and a step sized to the
    /// telemetry rate walks straight past its dynamics.
    pub fn advance(&mut self, condition: &Condition, duration: f64) -> Outputs {
        let steps = (duration / integrator::DT).round().max(1.0) as u32;

        for _ in 0..steps {
            self.wastegate = self.boost.update(
                &self.params,
                condition.fuel_cmd,
                self.state.p_im,
                self.state.omega_tc,
                integrator::DT,
            );
            self.propeller
                .govern(self.state.rpm(), condition.rpm_cmd, integrator::DT);

            let inputs = self.inputs(condition);
            let outputs = engine_model::evaluate(&self.params, &self.state, &inputs);
            self.fuel_burnt_m3 += outputs.w_fuel / self.params.fuel.density_kg_m3 * integrator::DT;
            self.state = step(&self.params, &self.state, &inputs, integrator::DT);
        }

        engine_model::evaluate(&self.params, &self.state, &self.inputs(condition))
    }

    fn inputs(&self, condition: &Condition) -> Inputs {
        Inputs {
            fuel_cmd: condition.fuel_cmd,
            wastegate: self.wastegate,
            p_amb: condition.p_amb,
            t_amb: condition.oat_k,
            tas_m_s: condition.tas_m_s,
            load_torque: self.propeller.absorbed_torque(
                &self.params,
                self.state.rpm(),
                condition.rho,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mission::Profile;

    /// The propeller law referred through the gearbox, against the published
    /// rating point: 550 N.m at the flange at 3880 crankshaft rpm.
    #[test]
    fn the_propeller_law_reproduces_the_rating_torque() {
        let p = engine_model::engines::ae330();
        let prop = Propeller::default();
        let rho = engine_model::atmosphere::isa(0.0).rho;
        let flange = prop.absorbed_torque(&p, 3880.0, rho) * p.geometry.gearbox_ratio;
        assert!((flange - 550.0).abs() / 550.0 < 0.15, "{flange} N.m");
    }

    /// Run the transient profile and sample the speed at the end of the
    /// high-fuelling window and again after the engine has been throttled back.
    fn run_transients() -> (f64, f64, f64) {
        let mut plant = Plant::new(
            engine_model::engines::ae330(),
            &Profile::Transients.condition_at(0.0),
        );
        let mut at_full_power = 0.0;
        for i in 0..1200 {
            let t = f64::from(i) * 0.05;
            let outputs = plant.advance(&Profile::Transients.condition_at(t), 0.05);
            assert!(outputs.power_brake_w.is_finite(), "diverged at {t} s");
            if (t - 39.0).abs() < 0.03 {
                at_full_power = plant.state.rpm();
            }
        }
        (
            at_full_power,
            plant.state.rpm(),
            plant.propeller.torque_coefficient,
        )
    }

    /// A governor that cannot hold speed makes every downstream residual
    /// meaningless, because then every channel moves whenever the engine drifts.
    #[test]
    fn the_governor_holds_commanded_speed_while_it_has_authority() {
        let (at_full_power, _, _) = run_transients();
        assert!(
            (at_full_power - 3600.0).abs() < 60.0,
            "held {at_full_power} rpm against a commanded 3600"
        );
    }

    /// Below the power that the finest available blade pitch absorbs, a
    /// constant-speed unit runs out of authority and the engine droops. That is
    /// what "on the fine stop" means on a real aircraft, and it matters here
    /// because a twin that assumed speed was always held would read the droop as
    /// a fault rather than as the governor doing the only thing it can.
    #[test]
    fn the_governor_droops_on_the_fine_stop_at_low_power() {
        let (_, throttled_back, pitch) = run_transients();
        assert!(throttled_back < 3600.0, "{throttled_back} rpm");
        assert!(
            (pitch - CQ_MIN).abs() < 1e-9,
            "pitch {pitch} is not at the stop"
        );
    }

    #[test]
    fn fuel_burnt_accumulates_monotonically() {
        let condition = Profile::Cruise.condition_at(0.0);
        let mut plant = Plant::new(engine_model::engines::ae330(), &condition);
        plant.advance(&condition, 1.0);
        let first = plant.fuel_burnt_m3;
        plant.advance(&condition, 1.0);
        assert!(plant.fuel_burnt_m3 > first);
        assert!(first > 0.0);
    }
}
