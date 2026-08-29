//! Engine parameter set, loaded from TOML.
//!
//! Parameters live in TOML and never in Rust literals, so that fitting the model to
//! a particular engine is editing a data file rather than editing code. Every entry
//! in the shipped parameter files is annotated `published` or `estimated`, because
//! the provenance of a constant is part of the model.
//!
//! Loading validates. A non-physical parameter set is rejected at startup, where
//! there is a person to read the message, rather than emitting `NaN` into a chart at
//! 20 Hz where there is not.

use serde::Deserialize;

/// A parameter set failed its physical-plausibility check.
#[derive(Debug, thiserror::Error)]
pub enum ParamError {
    /// The TOML did not parse or did not match the schema.
    #[error("parameter file did not parse: {0}")]
    Parse(#[from] toml::de::Error),
    /// A value is outside the range in which the model is meaningful.
    #[error("{field} = {value} is not physical: {reason}")]
    NotPhysical {
        /// Dotted path of the offending field.
        field: &'static str,
        /// The value that was rejected.
        value: f64,
        /// What the value would have broken.
        reason: &'static str,
    },
}

/// Bore, stroke, cylinder count and the reduction gearbox.
#[derive(Clone, Debug, Deserialize)]
pub struct Geometry {
    /// Total swept volume, m^3.
    pub displacement_m3: f64,
    /// Number of cylinders.
    pub n_cyl: f64,
    /// Crankshaft revolutions per thermodynamic cycle. 2 for a four-stroke.
    pub revs_per_cycle: f64,
    /// Cylinder bore, m.
    pub bore_m: f64,
    /// Piston stroke, m.
    pub stroke_m: f64,
    /// Geometric compression ratio.
    pub compression_ratio: f64,
    /// Crank speed divided by propeller speed.
    pub gearbox_ratio: f64,
    /// Rotating inertia referred to the crankshaft, kg m^2, including whatever the
    /// crankshaft is driving.
    pub inertia_kg_m2: f64,
}

/// Thermodynamic properties of the working gases.
#[derive(Clone, Debug, Deserialize)]
pub struct Gas {
    /// Specific gas constant of the intake charge, J/(kg K).
    pub r_air: f64,
    /// Specific heat at constant pressure of the intake charge, J/(kg K).
    pub cp_air: f64,
    /// Ratio of specific heats of the intake charge.
    pub gamma_air: f64,
    /// Specific gas constant of the exhaust gas, J/(kg K).
    pub r_exh: f64,
    /// Ratio of specific heats of the exhaust gas.
    pub gamma_exh: f64,
}

/// Fuel properties. Jet A-1 for a heavy-fuel engine, not diesel and not avgas.
#[derive(Clone, Debug, Deserialize)]
pub struct Fuel {
    /// Lower heating value, J/kg.
    pub lhv_j_per_kg: f64,
    /// Stoichiometric air/fuel mass ratio.
    pub stoich_afr: f64,
    /// Density at 15 C, kg/m^3. Used only to report volumetric fuel flow.
    pub density_kg_m3: f64,
}

/// Control-volume sizes and the exhaust outlet.
#[derive(Clone, Debug, Deserialize)]
pub struct Manifolds {
    /// Intake manifold volume including the intercooler outlet duct, m^3.
    pub v_im_m3: f64,
    /// Exhaust manifold volume, m^3.
    pub v_em_m3: f64,
    /// Effective flow area of the exhaust outlet, m^2. Stands in for the turbine
    /// when no turbocharger model is attached.
    pub exhaust_area_m2: f64,
}

/// Breathing, combustion efficiency and exhaust temperature.
#[derive(Clone, Debug, Deserialize)]
pub struct Cylinder {
    /// Volumetric efficiency coefficients `[c1, c2, c3]` for
    /// `eta_vol = c1 sqrt(p_im) + c2 sqrt(omega_e) + c3`.
    pub c_vol: [f64; 3],
    /// Upper clamp on volumetric efficiency.
    pub eta_vol_max: f64,
    /// Combustion-quality island `[eta at zero fuelling, peak eta, equivalence
    /// ratio at the peak]`.
    pub eta_ig_island: [f64; 3],
    /// In-cylinder ratio of specific heats as `[c0, c1]` in
    /// `gamma_cyl = c0 + c1 phi`, with `phi` the fuel/air equivalence ratio.
    pub gamma_cyl: [f64; 2],
    /// Exhaust temperature scaling factor.
    pub eta_sc: f64,
    /// Injected fuel mass per cylinder per cycle at full FADEC command, mg.
    pub u_f_max_mg: f64,
}

/// Rubbing and accessory losses.
#[derive(Clone, Debug, Deserialize)]
pub struct Friction {
    /// Friction mean effective pressure coefficients `[c1, c2, c3]` in bar, for
    /// `fmep = c1 n^2 + c2 n + c3` with `n` the crank speed in thousands of rpm.
    pub c_fr: [f64; 3],
}

/// Engine control set-points.
#[derive(Clone, Debug, Deserialize)]
pub struct Control {
    /// Intake manifold pressure the boost controller holds, Pa. Below the critical
    /// altitude the wastegate modulates to maintain it; above, it cannot be reached
    /// and power falls away. That inflection is the critical altitude.
    pub map_setpoint_pa: f64,
    /// Intercooler outlet temperature, K. Prescribed when no intercooler model is
    /// attached.
    pub t_im_k: f64,
}

/// Operating limits the model refuses to leave.
#[derive(Clone, Debug, Deserialize)]
pub struct Limits {
    /// Crankshaft overspeed limit, rpm.
    pub rpm_max: f64,
    /// Smoke-limit air/fuel excess ratio. Fuelling is clipped to hold this.
    pub lambda_min: f64,
}

/// A complete engine.
#[derive(Clone, Debug, Deserialize)]
pub struct EngineParams {
    /// Human-readable engine designation, shown in the UI chrome.
    pub name: String,
    /// Geometry block.
    pub geometry: Geometry,
    /// Gas properties block.
    pub gas: Gas,
    /// Fuel block.
    pub fuel: Fuel,
    /// Manifold block.
    pub manifolds: Manifolds,
    /// Cylinder block.
    pub cylinder: Cylinder,
    /// Friction block.
    pub friction: Friction,
    /// Control set-point block.
    pub control: Control,
    /// Limits block.
    pub limits: Limits,
}

impl EngineParams {
    /// Parse and validate a parameter set.
    ///
    /// # Errors
    /// Returns [`ParamError`] if the TOML does not match the schema or if a value
    /// would make the model non-physical.
    pub fn from_toml(s: &str) -> Result<Self, ParamError> {
        let p: Self = toml::from_str(s)?;
        p.validate()?;
        Ok(p)
    }

    fn validate(&self) -> Result<(), ParamError> {
        let positive: [(&'static str, f64, &'static str); 15] = [
            (
                "geometry.displacement_m3",
                self.geometry.displacement_m3,
                "divides the MEP relation",
            ),
            (
                "geometry.n_cyl",
                self.geometry.n_cyl,
                "divides the per-cylinder volume",
            ),
            (
                "geometry.revs_per_cycle",
                self.geometry.revs_per_cycle,
                "sets the fuel flow scale",
            ),
            (
                "geometry.gearbox_ratio",
                self.geometry.gearbox_ratio,
                "divides propeller speed",
            ),
            (
                "geometry.inertia_kg_m2",
                self.geometry.inertia_kg_m2,
                "divides the crank ODE",
            ),
            (
                "gas.r_air",
                self.gas.r_air,
                "divides the ideal-gas relation",
            ),
            (
                "gas.r_exh",
                self.gas.r_exh,
                "divides the ideal-gas relation",
            ),
            (
                "fuel.lhv_j_per_kg",
                self.fuel.lhv_j_per_kg,
                "scales all heat release",
            ),
            (
                "fuel.stoich_afr",
                self.fuel.stoich_afr,
                "divides the lambda relation",
            ),
            (
                "fuel.density_kg_m3",
                self.fuel.density_kg_m3,
                "divides volumetric fuel flow",
            ),
            (
                "manifolds.v_im_m3",
                self.manifolds.v_im_m3,
                "divides the filling ODE",
            ),
            (
                "manifolds.v_em_m3",
                self.manifolds.v_em_m3,
                "divides the filling ODE",
            ),
            (
                "limits.rpm_max",
                self.limits.rpm_max,
                "bounds the speed sweep",
            ),
            (
                "control.map_setpoint_pa",
                self.control.map_setpoint_pa,
                "sets the boost target",
            ),
            (
                "control.t_im_k",
                self.control.t_im_k,
                "divides the ideal-gas relation",
            ),
        ];
        for (field, value, reason) in positive {
            // Explicitly reject non-finite as well as non-positive: a NaN in a
            // parameter file survives every ordinary comparison and reappears as a
            // NaN in the state hours later, far from its cause.
            if !value.is_finite() || value <= 0.0 {
                return Err(ParamError::NotPhysical {
                    field,
                    value,
                    reason,
                });
            }
        }

        if self.geometry.compression_ratio <= 1.0 {
            return Err(ParamError::NotPhysical {
                field: "geometry.compression_ratio",
                value: self.geometry.compression_ratio,
                reason: "an ideal-cycle efficiency of zero or less",
            });
        }
        for (field, value) in [
            ("gas.gamma_air", self.gas.gamma_air),
            ("gas.gamma_exh", self.gas.gamma_exh),
            ("cylinder.gamma_cyl[0]", self.cylinder.gamma_cyl[0]),
        ] {
            if value <= 1.0 || value >= 2.0 {
                return Err(ParamError::NotPhysical {
                    field,
                    value,
                    reason: "a ratio of specific heats outside (1, 2) makes the flow \
                             function and the ideal-cycle efficiency imaginary",
                });
            }
        }
        if self.limits.lambda_min < 1.0 {
            return Err(ParamError::NotPhysical {
                field: "limits.lambda_min",
                value: self.limits.lambda_min,
                reason: "a compression-ignition engine cannot run rich of stoichiometric",
            });
        }
        // Bore and stroke are not used by any equation; they are here so the
        // schematic and the vibration orders can be drawn from the same source. If
        // they disagree with the displacement, one of the three is a typo.
        let swept = std::f64::consts::PI / 4.0
            * self.geometry.bore_m.powi(2)
            * self.geometry.stroke_m
            * self.geometry.n_cyl;
        if (swept - self.geometry.displacement_m3).abs() / self.geometry.displacement_m3 > 0.01 {
            return Err(ParamError::NotPhysical {
                field: "geometry.displacement_m3",
                value: self.geometry.displacement_m3,
                reason: "disagrees with bore, stroke and cylinder count by over 1%",
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::engines;

    #[test]
    fn the_shipped_engine_validates() {
        let p = engines::ae330();
        assert_eq!(p.geometry.n_cyl, 4.0);
    }

    #[test]
    fn inconsistent_geometry_is_rejected() {
        let bad = engines::AE330_TOML.replace("0.0830", "0.0900");
        assert!(super::EngineParams::from_toml(&bad).is_err());
    }
}
