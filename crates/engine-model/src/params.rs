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
    /// Heat-loss conductance of the exhaust manifold, W/K. The product of a heat
    /// transfer coefficient and a wetted area; only the product is identifiable.
    pub h_loss_w_per_k: f64,
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
    /// Injector flow rate at rail pressure, g/s. Converts a commanded mass into
    /// the injection duration an engine controller broadcasts.
    pub injector_flow_g_per_s: f64,
    /// Per-cylinder injector flow scale. Unity on a healthy engine; this is the
    /// parameter an injector fault acts on.
    ///
    /// Optional in a parameter file, because a file describes an engine and unity
    /// is what a healthy engine has. See [`unity`].
    #[serde(default = "unity")]
    pub injector_scale: [f64; crate::CYLINDERS],
    /// Per-cylinder fraction of the delivered fuel that releases its heat. Unity
    /// on a healthy engine; this is the parameter misfire acts on.
    ///
    /// Deliberately separate from `injector_scale`, and the pair is what makes an
    /// injection fault distinguishable from a combustion one. A restricted nozzle
    /// never puts the fuel in the cylinder, so total fuel flow falls with the
    /// torque; a misfiring cylinder is fuelled normally and burns none of it, so
    /// torque falls at unchanged fuel flow. Every other channel moves the same way
    /// for both, which is why a threshold monitor cannot separate them.
    ///
    /// Optional in a parameter file, for the same reason as `injector_scale`.
    #[serde(default = "unity")]
    pub combustion_efficiency: [f64; crate::CYLINDERS],
}

/// A per-cylinder scale with every cylinder at nominal.
///
/// The serde default for both per-cylinder arrays. Neither describes the engine:
/// they exist so a fault can perturb one cylinder, and a file that says nothing
/// about them is describing a healthy engine rather than an underspecified one.
/// Making them optional also keeps this schema additive, which matters because
/// adding a required field to a published crate breaks every file already written
/// against it.
fn unity() -> [f64; crate::CYLINDERS] {
    [1.0; crate::CYLINDERS]
}

/// Rubbing and accessory losses.
#[derive(Clone, Debug, Deserialize)]
pub struct Friction {
    /// Friction mean effective pressure coefficients `[c1, c2, c3]` in bar, for
    /// `fmep = c1 n^2 + c2 n + c3` with `n` the crank speed in thousands of rpm.
    pub c_fr: [f64; 3],
}

/// Centrifugal compressor and the intercooler behind it.
#[derive(Clone, Debug, Deserialize)]
pub struct Compressor {
    /// Compressor wheel outer radius, m. Sets the head at a given shaft speed.
    pub r_wheel_m: f64,
    /// Dimensionless head coefficient at the top of the ellipse.
    pub psi_max: f64,
    /// Shaft speed used to normalise corrected speed, rad/s.
    pub omega_ref: f64,
    /// Inlet temperature the map is corrected to, K.
    pub t_ref_k: f64,
    /// Inlet pressure the map is corrected to, Pa.
    pub p_ref_pa: f64,
    /// Choke corrected mass flow as `[a, b, c]` in `a n^2 + b n + c`, with `n` the
    /// normalised corrected speed. kg/s.
    pub m_corr_max: [f64; 3],
    /// Peak isentropic efficiency.
    pub eta_max: f64,
    /// Flow coefficient at peak efficiency.
    pub phi_opt: f64,
    /// Normalised corrected speed at peak efficiency.
    pub omega_norm_opt: f64,
    /// Quadratic form `[q_phi, q_speed, q_cross]` for the efficiency fall-off.
    pub q_form: [f64; 3],
    /// Surge line as `[slope, intercept]` in corrected flow.
    pub surge: [f64; 2],
    /// Intercooler effectiveness, 0 to 1.
    pub intercooler_effectiveness: f64,
}

/// Radial turbine.
#[derive(Clone, Debug, Deserialize)]
pub struct Turbine {
    /// Effective flow area, m^2. The primary lever on boost and therefore on where
    /// the critical altitude lands.
    pub area_eff_m2: f64,
    /// Flow function coefficients `[a, b]` in `a sqrt(1 - Pi^b)`.
    pub c_flow: [f64; 2],
    /// Turbine wheel radius, m.
    pub r_wheel_m: f64,
    /// Blade speed ratio at peak efficiency.
    pub bsr_opt: f64,
    /// Peak combined isentropic and mechanical efficiency.
    pub eta_max: f64,
    /// Curvature of the efficiency parabola in blade speed ratio.
    pub c_bsr: f64,
}

/// Wastegate.
#[derive(Clone, Debug, Deserialize)]
pub struct Wastegate {
    /// Effective flow area when fully open, m^2.
    pub area_eff_m2: f64,
    /// Flow function coefficients `[a, b]` in `a sqrt(1 - Pi^b)`.
    pub c_flow: [f64; 2],
}

/// The shaft the compressor and turbine share.
#[derive(Clone, Debug, Deserialize)]
pub struct Turbocharger {
    /// Rotor polar moment of inertia, kg m^2.
    pub inertia_kg_m2: f64,
    /// Speed the rotor is certified to contain, rad/s.
    pub omega_max: f64,
    /// Speed floor, rad/s. The shaft power balance divides by speed, so it cannot be
    /// allowed to reach zero.
    pub omega_min: f64,
}

/// Engine control set-points.
#[derive(Clone, Debug, Deserialize)]
pub struct Control {
    /// Intake manifold pressure the boost controller holds at **full** fuelling, Pa.
    /// Below the critical altitude the wastegate modulates to maintain it; above, it
    /// cannot be reached and power falls away. That inflection is the critical altitude.
    pub map_setpoint_pa: f64,
    /// The boost schedule extrapolated to **zero** fuelling, Pa.
    ///
    /// The set point is linear in fuelling demand between this and
    /// [`Control::map_setpoint_pa`]. A production controller uses a two-dimensional
    /// map fitted on a dynamometer; this is that map reduced to its dominant axis.
    ///
    /// It sits **below any ambient pressure on purpose**. That is what makes the
    /// controller demand no boost at all at low fuelling and hold the wastegate open,
    /// which is what a turbocharged diesel does at idle. Treating it as a pressure the
    /// engine ever runs at is a misreading; it is the intercept of a line.
    pub map_setpoint_zero_pa: f64,
    /// Boost controller proportional gain, wastegate command per Pa of error.
    pub kp: f64,
    /// Boost controller integral gain, wastegate command per Pa-second of error.
    pub ki: f64,
    /// Anti-windup clamp on the boost controller integrator, Pa-seconds.
    pub integral_limit: f64,
    /// Turbocharger speed the controller holds the shaft below, rad/s. Set under
    /// the certified containment speed, not at it.
    pub turbo_omega_limit: f64,
    /// Overspeed loop gain, wastegate command per rad/s of exceedance.
    pub kp_overspeed: f64,
}

/// Cylinder head metal nodes.
#[derive(Clone, Debug, Deserialize)]
pub struct Thermal {
    /// Fraction of a cylinder's fuel energy that enters its head metal.
    pub heat_fraction_to_head: f64,
    /// Head-to-coolant conductance at the maximum engine speed, W/K.
    pub head_conductance_w_per_k: f64,
    /// Thermal capacity of one cylinder head node, J/K.
    pub head_capacity_j_per_k: f64,
}

/// Coolant circuit and radiator.
#[derive(Clone, Debug, Deserialize)]
pub struct Cooling {
    /// Radiator frontal area, m^2.
    pub radiator_area_m2: f64,
    /// Radiator effectiveness, 0 to 1.
    pub radiator_effectiveness: f64,
    /// Fraction of the approaching stream that passes through a core rather than
    /// spilling around it. Parsons & Harper give 0.3 to 0.7 for ordinary cores.
    pub air_flow_constant: f64,
    /// Thermal capacity of the coolant and the metal it is coupled to, J/K.
    pub coolant_capacity_j_per_k: f64,
    /// Coolant temperature at which the thermostat starts to open, K.
    pub thermostat_open_k: f64,
    /// Temperature span over which it opens fully, K.
    pub thermostat_band_k: f64,
    /// Radiator flow admitted with the thermostat shut. Never zero.
    pub bypass_fraction: f64,
}

/// Lubrication circuit.
#[derive(Clone, Debug, Deserialize)]
pub struct Oil {
    /// Vogel viscosity coefficients `[a, b, c]` in `a exp(b / (T - c))`.
    pub vogel: [f64; 3],
    /// Pump displacement divided by leakage conductance. Only the group is
    /// identifiable, so it is carried as one number.
    pub pressure_coefficient: f64,
    /// Relief valve setting, Pa above ambient.
    pub relief_pressure_pa: f64,
    /// Fraction of fuel energy reaching the oil through the piston-cooling jets.
    pub heat_fraction_from_fuel: f64,
    /// Thermal capacity of the oil charge and its sump, J/K.
    pub capacity_j_per_k: f64,
    /// Oil cooler frontal area, m^2.
    pub cooler_area_m2: f64,
    /// Oil cooler effectiveness, 0 to 1.
    pub cooler_effectiveness: f64,
    /// Oil temperature at which the cooler thermostat starts to open, K.
    pub thermostat_open_k: f64,
    /// Temperature span over which it opens fully, K.
    pub thermostat_band_k: f64,
    /// Cooler flow admitted with the thermostat shut.
    pub bypass_fraction: f64,
}

/// Operating limits the model refuses to leave.
#[derive(Clone, Debug, Deserialize)]
pub struct Limits {
    /// Crankshaft overspeed limit, rpm.
    pub rpm_max: f64,
    /// Smoke-limit air/fuel excess ratio. Fuelling is clipped to hold this.
    pub lambda_min: f64,
    /// Rated brake power, W. The denominator an engine controller divides by when
    /// it broadcasts engine load as a percentage.
    pub rated_power_w: f64,
    /// The certified redline set: what a conventional threshold monitor watches.
    pub redline: Redline,
}

/// Operating limits a conventional cockpit gauge or threshold monitor alarms on.
///
/// These are here so the twin can be compared against the monitor it replaces on
/// the same engine and the same run, which is what turns "earlier than a threshold"
/// from a claim into a measured lead time. They are limits, not model parameters:
/// nothing in the physics reads them, and crossing one is a statement about the
/// aircraft rather than about the equations.
///
/// Oil and coolant limits are **published**, from EASA TCDS E.200 section IV for the
/// E4P. Exhaust and head limits are **estimated**: the certificate publishes neither,
/// because on this engine they are managed by the EECU's own fuelling limiter rather
/// than presented to a pilot.
#[derive(Clone, Copy, Debug, Deserialize)]
pub struct Redline {
    /// Maximum oil temperature, K. **published**, 139 C.
    pub oil_t_max_k: f64,
    /// Minimum oil pressure at maximum continuous power, Pa. **published**, 2.5 bar.
    pub oil_p_min_pa: f64,
    /// Maximum oil pressure, Pa. **published**, 6.5 bar.
    pub oil_p_max_pa: f64,
    /// Maximum coolant temperature, K. **published**, 100 C.
    pub coolant_t_max_k: f64,
    /// Maximum exhaust gas temperature at any cylinder, K. **estimated**.
    pub egt_max_k: f64,
    /// Maximum cylinder head temperature, K. **estimated**.
    pub cht_max_k: f64,
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
    /// Compressor block.
    pub compressor: Compressor,
    /// Turbine block.
    pub turbine: Turbine,
    /// Wastegate block.
    pub wastegate: Wastegate,
    /// Turbocharger shaft block.
    pub turbocharger: Turbocharger,
    /// Cylinder head thermal block.
    pub thermal: Thermal,
    /// Coolant circuit block.
    pub cooling: Cooling,
    /// Lubrication block.
    pub oil: Oil,
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
        let positive: [(&'static str, f64, &'static str); 36] = [
            (
                "thermal.head_conductance_w_per_k",
                self.thermal.head_conductance_w_per_k,
                "divides the head balance",
            ),
            (
                "thermal.head_capacity_j_per_k",
                self.thermal.head_capacity_j_per_k,
                "divides the head ODE",
            ),
            (
                "cooling.radiator_area_m2",
                self.cooling.radiator_area_m2,
                "scales radiator flow",
            ),
            (
                "cooling.coolant_capacity_j_per_k",
                self.cooling.coolant_capacity_j_per_k,
                "divides the coolant ODE",
            ),
            (
                "cooling.thermostat_band_k",
                self.cooling.thermostat_band_k,
                "divides the thermostat ramp",
            ),
            (
                "oil.pressure_coefficient",
                self.oil.pressure_coefficient,
                "scales gallery pressure",
            ),
            (
                "oil.relief_pressure_pa",
                self.oil.relief_pressure_pa,
                "caps gallery pressure",
            ),
            (
                "oil.capacity_j_per_k",
                self.oil.capacity_j_per_k,
                "divides the oil ODE",
            ),
            (
                "oil.cooler_area_m2",
                self.oil.cooler_area_m2,
                "scales oil cooler flow",
            ),
            (
                "oil.thermostat_band_k",
                self.oil.thermostat_band_k,
                "divides the thermostat ramp",
            ),
            (
                "manifolds.h_loss_w_per_k",
                self.manifolds.h_loss_w_per_k,
                "scales exhaust cooling",
            ),
            (
                "compressor.r_wheel_m",
                self.compressor.r_wheel_m,
                "squares into the head",
            ),
            (
                "compressor.omega_ref",
                self.compressor.omega_ref,
                "normalises corrected speed",
            ),
            (
                "compressor.t_ref_k",
                self.compressor.t_ref_k,
                "divides the speed correction",
            ),
            (
                "compressor.p_ref_pa",
                self.compressor.p_ref_pa,
                "divides the flow correction",
            ),
            (
                "compressor.eta_max",
                self.compressor.eta_max,
                "divides the compressor power",
            ),
            (
                "turbine.area_eff_m2",
                self.turbine.area_eff_m2,
                "scales turbine flow",
            ),
            (
                "turbine.r_wheel_m",
                self.turbine.r_wheel_m,
                "scales blade speed ratio",
            ),
            (
                "wastegate.area_eff_m2",
                self.wastegate.area_eff_m2,
                "scales the bypass flow",
            ),
            (
                "turbocharger.inertia_kg_m2",
                self.turbocharger.inertia_kg_m2,
                "divides the shaft ODE",
            ),
            (
                "turbocharger.omega_min",
                self.turbocharger.omega_min,
                "keeps the shaft balance finite",
            ),
            (
                "turbocharger.omega_max",
                self.turbocharger.omega_max,
                "bounds the shaft speed",
            ),
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
        if (self.geometry.n_cyl - crate::CYLINDERS as f64).abs() > f64::EPSILON {
            return Err(ParamError::NotPhysical {
                field: "geometry.n_cyl",
                value: self.geometry.n_cyl,
                reason: "the per-cylinder channels are fixed-width arrays, so this \
                         model is four-cylinder; see CYLINDERS",
            });
        }
        if self.control.map_setpoint_zero_pa >= self.control.map_setpoint_pa {
            return Err(ParamError::NotPhysical {
                field: "control.map_setpoint_zero_pa",
                value: self.control.map_setpoint_zero_pa,
                reason: "the boost schedule would run backwards, so more fuel would \
                         demand less air",
            });
        }
        if self.limits.lambda_min < 1.0 {
            return Err(ParamError::NotPhysical {
                field: "limits.lambda_min",
                value: self.limits.lambda_min,
                reason: "a compression-ignition engine cannot run rich of stoichiometric",
            });
        }
        // The per-cylinder arrays, which the scalar sweep above cannot reach.
        //
        // These are the two values a fault moves, so they are the two most likely to
        // be edited by hand, and a bad one is invisible until it is far from its
        // cause: a non-finite scale multiplies into the fuel mass, survives the
        // excess-air ratio because a NaN compares false against every bound, and
        // surfaces as a NaN brake torque on the wire. Zero is allowed on both, and
        // deliberately: a totally blocked nozzle and a dead cylinder are the
        // extremes of real faults, and the model answers for them.
        for i in 0..crate::CYLINDERS {
            let scale = self.cylinder.injector_scale[i];
            if !scale.is_finite() || scale < 0.0 {
                return Err(ParamError::NotPhysical {
                    field: "cylinder.injector_scale",
                    value: scale,
                    reason: "an injector cannot deliver a negative mass of fuel",
                });
            }
            let burned = self.cylinder.combustion_efficiency[i];
            if !burned.is_finite() || !(0.0..=1.0).contains(&burned) {
                return Err(ParamError::NotPhysical {
                    field: "cylinder.combustion_efficiency",
                    value: burned,
                    reason: "a cylinder cannot release less heat than none or more \
                             than the fuel it was given holds",
                });
            }
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

    /// The schedule must slope the right way. Inverted, the controller would
    /// quietly demand less air for more fuel, which shows up as a smoke-limited
    /// engine rather than as a parameter error.
    #[test]
    fn an_inverted_boost_schedule_is_rejected() {
        let toml = engines::AE330_TOML.replace(
            "map_setpoint_zero_pa = 0.20e5",
            "map_setpoint_zero_pa = 4.0e5",
        );
        assert!(crate::EngineParams::from_toml(&toml).is_err());
    }

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

    /// A file that says nothing about the per-cylinder scales describes a healthy
    /// engine, and has to load. Both arrays were added after the first parameter
    /// file was written, and making either one required would reject every file
    /// already written against this schema.
    #[test]
    fn a_file_that_omits_the_per_cylinder_scales_loads_as_healthy() {
        let older = engines::AE330_TOML
            .replace("injector_scale = [1.0, 1.0, 1.0, 1.0] # estimated", "")
            .replace(
                "combustion_efficiency = [1.0, 1.0, 1.0, 1.0] # estimated",
                "",
            );
        let p = super::EngineParams::from_toml(&older).expect("an older file must load");
        assert_eq!(p.cylinder.injector_scale, [1.0; crate::CYLINDERS]);
        assert_eq!(p.cylinder.combustion_efficiency, [1.0; crate::CYLINDERS]);
    }

    /// The scalar sweep in `validate` cannot reach an array, and these two arrays
    /// are the ones a hand edit is most likely to touch because they are what a
    /// fault moves. A non-finite entry is the dangerous case: it compares false
    /// against every bound, multiplies into the fuel mass and surfaces as a NaN
    /// brake torque on the wire, hours from its cause.
    #[test]
    fn a_per_cylinder_scale_outside_its_range_is_rejected() {
        let cases = [
            (
                "injector_scale = [1.0, 1.0, 1.0, 1.0]",
                "[1.0, nan, 1.0, 1.0]",
            ),
            (
                "injector_scale = [1.0, 1.0, 1.0, 1.0]",
                "[1.0, -0.2, 1.0, 1.0]",
            ),
            (
                "combustion_efficiency = [1.0, 1.0, 1.0, 1.0]",
                "[1.0, 1.0, nan, 1.0]",
            ),
            (
                "combustion_efficiency = [1.0, 1.0, 1.0, 1.0]",
                "[1.0, 1.0, 1.4, 1.0]",
            ),
        ];
        for (needle, bad) in cases {
            let field = needle.split(' ').next().expect("a field name");
            let toml = engines::AE330_TOML.replace(needle, &format!("{field} = {bad}"));
            assert!(
                super::EngineParams::from_toml(&toml).is_err(),
                "{field} = {bad} was accepted"
            );
        }
    }

    /// The extremes are faults, not errors: a totally blocked nozzle and a cylinder
    /// that never lights are both reachable, and the model answers for them.
    #[test]
    fn a_dead_cylinder_is_a_fault_rather_than_a_bad_parameter_file() {
        for (needle, extreme) in [
            (
                "injector_scale = [1.0, 1.0, 1.0, 1.0]",
                "[1.0, 0.0, 1.0, 1.0]",
            ),
            (
                "combustion_efficiency = [1.0, 1.0, 1.0, 1.0]",
                "[1.0, 1.0, 0.0, 1.0]",
            ),
        ] {
            let field = needle.split(' ').next().expect("a field name");
            let toml = engines::AE330_TOML.replace(needle, &format!("{field} = {extreme}"));
            assert!(
                super::EngineParams::from_toml(&toml).is_ok(),
                "{field} = {extreme} was rejected"
            );
        }
    }
}
