//! Everything the model computes at one operating point.
//!
//! Deliberately wide, and separate from the state on purpose. Diagnosis works on the
//! disagreement between a measurement and the corresponding modelled quantity, so
//! anything a sensor might report has to be here to be compared against. State
//! variables are not duplicated: a caller that has the outputs also has the state
//! that produced them.

use crate::{CYLINDERS, EngineParams};

/// Modelled values at one operating point.
///
/// Every field here has either a consumer today or a direct analogue in the
/// telemetry a real engine controller broadcasts. Quantities that are neither, such
/// as in-cylinder gas temperature before the manifold cools it or the compressor
/// pressure ratio that is one division away from two fields already present, are
/// deliberately absent. They are cheap to add when something needs them and they are
/// noise until then.
#[derive(Clone, Copy, Debug)]
pub struct Outputs {
    /// Intake charge temperature after the intercooler, K.
    pub t_intake: f64,
    /// Compressor mass flow, kg/s.
    pub w_compressor: f64,
    /// Compressor isentropic efficiency.
    pub eta_compressor: f64,
    /// Shaft power absorbed by the compressor, W.
    pub power_compressor: f64,
    /// Margin to the compressor surge line, in pressure-ratio units.
    pub surge_margin: f64,
    /// Volumetric efficiency.
    pub eta_vol: f64,
    /// Air mass flow into the cylinders, kg/s.
    pub w_air: f64,
    /// Commanded injection quantity per cylinder per cycle after the smoke limit, mg.
    pub u_f_mg: f64,
    /// Delivered injection quantity per cylinder, mg, after the injector scales.
    pub u_f_cylinder: [f64; CYLINDERS],
    /// Total fuel mass flow, kg/s.
    pub w_fuel: f64,
    /// Fuel mass flow per cylinder, kg/s.
    pub w_fuel_cylinder: [f64; CYLINDERS],
    /// Excess air ratio over all cylinders. Infinite while motoring.
    pub lambda: f64,
    /// Excess air ratio per cylinder.
    pub lambda_cylinder: [f64; CYLINDERS],
    /// Gross indicated efficiency per cylinder.
    pub eta_ig: [f64; CYLINDERS],
    /// Gross indicated torque, N.m.
    pub torque_indicated: f64,
    /// Friction and accessory torque, N.m.
    pub torque_friction: f64,
    /// Pumping torque, N.m. Negative when boost exceeds back pressure.
    pub torque_pumping: f64,
    /// Brake torque at the crankshaft, N.m.
    pub torque_brake: f64,
    /// Brake power, W.
    pub power_brake_w: f64,
    /// Torque at the propeller flange, N.m.
    pub torque_prop: f64,
    /// Propeller speed, rpm.
    pub rpm_prop: f64,
    /// Exhaust gas temperature per cylinder as a manifold thermocouple reads it, K.
    /// Cooler than [`Outputs::t_cylinder_out`], which is why the heat-loss term
    /// exists.
    pub t_egt: [f64; CYLINDERS],
    /// Mixed exhaust temperature reaching the turbine, K.
    pub t_exhaust: f64,
    /// Mass flow through the turbine, kg/s.
    pub w_turbine: f64,
    /// Mass flow bypassing the turbine, kg/s.
    pub w_wastegate: f64,
    /// Turbine blade speed ratio.
    pub blade_speed_ratio: f64,
    /// Turbine combined isentropic and mechanical efficiency.
    pub eta_turbine: f64,
    /// Shaft power delivered by the turbine, W.
    pub power_turbine: f64,
    /// Heat leaving the cylinder heads into the coolant, W.
    pub heat_to_coolant: f64,
    /// Heat entering the oil from friction and piston cooling, W.
    pub heat_to_oil: f64,
    /// Oil gallery pressure above ambient, Pa.
    pub p_oil: f64,
}

impl Outputs {
    /// Brake specific fuel consumption, g/kWh.
    ///
    /// `None` while the engine is motoring, because specific consumption is
    /// undefined at or below zero output. Returning a sentinel instead would put a
    /// number on a display that means the opposite of what it appears to.
    #[must_use]
    pub fn bsfc_g_per_kwh(&self) -> Option<f64> {
        (self.power_brake_w > 0.0).then(|| self.w_fuel * 3.6e9 / self.power_brake_w)
    }

    /// Volumetric fuel flow, litres/hour.
    #[must_use]
    pub fn fuel_litres_per_hour(&self, p: &EngineParams) -> f64 {
        self.w_fuel / p.fuel.density_kg_m3 * 3.6e6
    }

    /// Mean gross indicated efficiency across the cylinders.
    #[must_use]
    pub fn eta_ig_mean(&self) -> f64 {
        self.eta_ig.iter().sum::<f64>() / CYLINDERS as f64
    }
}
