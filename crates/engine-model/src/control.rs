//! The boost controller.
//!
//! Separate from the plant on purpose. `engine-model` models an engine, not an engine
//! plus its FADEC, so the wastegate command is an input to the model and this
//! controller is an object a caller may choose to drive it with. Keeping the
//! controller's integrator out of the state vector also means the twin can run the
//! plant open-loop against a wastegate position read from telemetry, which is what
//! residual generation needs.
//!
//! Only the boost loop lives here. A real FADEC also schedules injection timing, rail
//! pressure, glow plugs and the redundancy logic, and none of that is modelled.

use crate::EngineParams;

/// Boost controller with turbocharger overspeed protection.
#[derive(Clone, Copy, Debug, Default)]
pub struct BoostController {
    integral: f64,
}

impl BoostController {
    /// A controller with a cleared integrator.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Compute the wastegate command, 0 shut to 1 fully open, for one step.
    ///
    /// Two loops, and the more open of the two wins.
    ///
    /// The boost loop is a PI, and its sign is inverted relative to the obvious:
    /// **too much** boost opens the wastegate, dumping energy away from the turbine.
    /// Anti-windup is clamping rather than back-calculation, which is enough because
    /// the actuator saturates at both ends and the loop is not required to be fast;
    /// without it the integrator charges through the whole climb to critical
    /// altitude and the wastegate then refuses to reopen on descent.
    ///
    /// The overspeed loop is proportional only, and it exists because above the
    /// critical altitude the expansion ratio across the turbine keeps growing while
    /// the wastegate is already shut. Left alone the shaft would run past its
    /// containment speed, so a real engine controller supervises it and bleeds
    /// exhaust away. Without this the only thing holding the shaft back would be a
    /// numerical clamp in the integrator, which would mean an arbitrary limit rather
    /// than a control law was setting how the engine behaves at altitude.
    pub fn update(&mut self, p: &EngineParams, p_im: f64, omega_tc: f64, dt: f64) -> f64 {
        let c = &p.control;
        let error = p_im - c.map_setpoint_pa;
        let boost = c.kp * error + c.ki * self.integral;
        if boost > 0.0 && boost < 1.0 {
            self.integral += error * dt;
        }
        self.integral = self.integral.clamp(-c.integral_limit, c.integral_limit);

        let overspeed = c.kp_overspeed * (omega_tc - c.turbo_omega_limit);
        boost.max(overspeed).clamp(0.0, 1.0)
    }

    /// The integrator state, for inspection and for seeding a replay.
    #[must_use]
    pub fn integral(&self) -> f64 {
        self.integral
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engines;

    #[test]
    fn over_boost_opens_the_wastegate_and_under_boost_shuts_it() {
        let p = engines::ae330();
        let mut c = BoostController::new();
        assert!(c.update(&p, p.control.map_setpoint_pa + 5e4, 5_000.0, 0.005) > 0.0);
        let mut c = BoostController::new();
        assert!(
            c.update(&p, p.control.map_setpoint_pa - 5e4, 5_000.0, 0.005)
                .abs()
                < 1e-12
        );
    }

    #[test]
    fn the_integrator_does_not_wind_up_while_saturated() {
        let p = engines::ae330();
        let mut c = BoostController::new();
        // Hold it hard against the shut stop for a simulated minute, as a long climb
        // to critical altitude would, then ask for a big overshoot.
        for _ in 0..12_000 {
            c.update(&p, p.control.map_setpoint_pa - 2e5, 5_000.0, 0.005);
        }
        assert!(c.integral().abs() <= p.control.integral_limit + 1e-9);
        assert!(c.update(&p, p.control.map_setpoint_pa + 1e5, 5_000.0, 0.005) > 0.0);
    }

    #[test]
    fn overspeed_opens_the_wastegate_even_when_boost_is_low() {
        // Above the critical altitude the boost loop is calling for a shut
        // wastegate and the shaft is still accelerating. The overspeed loop has to
        // win, or a numerical clamp ends up deciding how the engine behaves.
        let p = engines::ae330();
        let mut c = BoostController::new();
        let under_boost = p.control.map_setpoint_pa - 1e5;
        assert!(c.update(&p, under_boost, p.control.turbo_omega_limit - 500.0, 0.005) < 1e-12);
        assert!(c.update(&p, under_boost, p.control.turbo_omega_limit + 2000.0, 0.005) > 0.0);
    }
}
