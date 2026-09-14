/**
 * The engine parameter ledger: every constant the model runs on, with its source.
 *
 * Extracted mechanically from `crates/engine-model/src/engines/ae330.toml` rather
 * than retyped, because a provenance display that miscopies a value is worse than
 * no display. `value` is the literal as the file writes it.
 *
 * `published` means a manufacturer, a type certificate or a materials standard
 * states it. `estimated` means nobody publishes it, it was chosen from engine
 * class, and it was then fitted so the model reproduces the one rating point that
 * is published. The distinction is the point of the file: a reader who wants to
 * attack a number should be told which ones are attackable.
 *
 * MOCK: a snapshot, not a live read. The screen is scheduled to move to the landing
 * page, so it is not worth a generator; re-extract if the TOML changes.
 */

export interface Parameter {
  /** TOML table the entry sits in. */
  section: string;
  key: string;
  /** The literal, as written in the parameter file. */
  value: string;
  source: "published" | "estimated";
  /** The file's own annotation, where it carries one past the tag. */
  note?: string;
}

export const PARAMETERS: Parameter[] = [
  {
    section: "geometry",
    key: "displacement_m3",
    value: "1.991e-3",
    source: "published",
    note: "1991 cm3",
  },
  { section: "geometry", key: "n_cyl", value: "4.0", source: "published" },
  {
    section: "geometry",
    key: "revs_per_cycle",
    value: "2.0",
    source: "published",
    note: "four-stroke",
  },
  { section: "geometry", key: "bore_m", value: "0.0830", source: "published", note: "OM640 block" },
  {
    section: "geometry",
    key: "stroke_m",
    value: "0.0920",
    source: "published",
    note: "OM640 block",
  },
  {
    section: "geometry",
    key: "compression_ratio",
    value: "18.0",
    source: "published",
    note: "OM640 block",
  },
  {
    section: "geometry",
    key: "gearbox_ratio",
    value: "1.69",
    source: "published",
    note: "crank rpm per propeller rpm",
  },
  { section: "geometry", key: "inertia_kg_m2", value: "0.55", source: "estimated" },
  { section: "gas", key: "r_air", value: "287.05", source: "published", note: "dry air" },
  { section: "gas", key: "cp_air", value: "1011.0", source: "published" },
  { section: "gas", key: "gamma_air", value: "1.3964", source: "published" },
  { section: "gas", key: "r_exh", value: "286.0", source: "published", note: "burned gas" },
  { section: "gas", key: "gamma_exh", value: "1.2734", source: "published", note: "burned gas" },
  {
    section: "fuel",
    key: "lhv_j_per_kg",
    value: "43.0e6",
    source: "published",
    note: "ASTM D1655 specification energy content",
  },
  {
    section: "fuel",
    key: "stoich_afr",
    value: "14.7",
    source: "published",
    note: "kerosene stoichiometric air/fuel mass ratio",
  },
  {
    section: "fuel",
    key: "density_kg_m3",
    value: "800.0",
    source: "published",
    note: "nominal at 15 C, spec range 775 to 840",
  },
  {
    section: "manifolds",
    key: "v_im_m3",
    value: "3.0e-3",
    source: "estimated",
    note: "includes the intercooler outlet duct",
  },
  { section: "manifolds", key: "v_em_m3", value: "1.0e-3", source: "estimated" },
  { section: "manifolds", key: "h_loss_w_per_k", value: "23.0", source: "estimated" },
  {
    section: "cylinder",
    key: "c_vol",
    value: "[6.0e-5, -0.0125, 1.1400]",
    source: "estimated",
    note: "fitted to the rating point",
  },
  {
    section: "cylinder",
    key: "eta_vol_max",
    value: "1.00",
    source: "estimated",
    note: "see the clamp note in cylinder.rs",
  },
  { section: "cylinder", key: "eta_ig_island", value: "[0.45, 0.760, 0.55]", source: "estimated" },
  { section: "cylinder", key: "gamma_cyl", value: "[1.35, -0.15]", source: "estimated" },
  { section: "cylinder", key: "eta_sc", value: "1.21", source: "estimated" },
  { section: "cylinder", key: "u_f_max_mg", value: "67.4", source: "estimated" },
  {
    section: "cylinder",
    key: "injector_flow_g_per_s",
    value: "17.0",
    source: "estimated",
    note: "common-rail solenoid nozzle at 1600 bar",
  },
  {
    section: "cylinder",
    key: "injector_scale",
    value: "[1.0, 1.0, 1.0, 1.0]",
    source: "estimated",
  },
  {
    section: "cylinder",
    key: "combustion_efficiency",
    value: "[1.0, 1.0, 1.0, 1.0]",
    source: "estimated",
  },
  { section: "friction", key: "c_fr", value: "[0.07822, 0.12178, 0.55]", source: "estimated" },
  {
    section: "compressor",
    key: "r_wheel_m",
    value: "0.0285",
    source: "estimated",
    note: "57 mm wheel; sized so rated power holds to 11,000 ft",
  },
  {
    section: "compressor",
    key: "psi_max",
    value: "1.65",
    source: "estimated",
    note: "dimensionless head coefficient",
  },
  {
    section: "compressor",
    key: "omega_ref",
    value: "18000.0",
    source: "estimated",
    note: "normalisation near the containment speed",
  },
  {
    section: "compressor",
    key: "t_ref_k",
    value: "288.15",
    source: "published",
    note: "ISA sea level, the correction reference",
  },
  {
    section: "compressor",
    key: "p_ref_pa",
    value: "101325.0",
    source: "published",
    note: "ISA sea level, the correction reference",
  },
  {
    section: "compressor",
    key: "m_corr_max",
    value: "[0.0252, 0.2053, 0.1153]",
    source: "estimated",
    note: "choke flow vs normalised speed",
  },
  {
    section: "compressor",
    key: "eta_max",
    value: "0.78",
    source: "estimated",
    note: "peak for a small automotive-class compressor",
  },
  {
    section: "compressor",
    key: "phi_opt",
    value: "0.0600",
    source: "estimated",
    note: "flow coefficient at the efficiency peak",
  },
  { section: "compressor", key: "omega_norm_opt", value: "0.78", source: "estimated" },
  {
    section: "compressor",
    key: "q_form",
    value: "[79.27, 0.7985, -1.4358]",
    source: "estimated",
    note: "efficiency island shape",
  },
  {
    section: "compressor",
    key: "surge",
    value: "[14.5, 0.60]",
    source: "estimated",
    note: "placed to give the design envelope a plausible margin",
  },
  {
    section: "compressor",
    key: "intercooler_effectiveness",
    value: "0.77",
    source: "estimated",
    note: "air to air, ram cooled",
  },
  {
    section: "turbine",
    key: "area_eff_m2",
    value: "4.95e-4",
    source: "estimated",
    note: "the primary lever on where the knee falls",
  },
  {
    section: "turbine",
    key: "c_flow",
    value: "[0.6859, 2.3633]",
    source: "estimated",
    note: "radial turbine flow function",
  },
  {
    section: "turbine",
    key: "r_wheel_m",
    value: "0.020",
    source: "estimated",
    note: "40 mm wheel",
  },
  {
    section: "turbine",
    key: "bsr_opt",
    value: "0.622",
    source: "estimated",
    note: "blade speed ratio at peak efficiency",
  },
  {
    section: "turbine",
    key: "eta_max",
    value: "0.7075",
    source: "estimated",
    note: "isentropic times mechanical",
  },
  {
    section: "turbine",
    key: "c_bsr",
    value: "2.0245",
    source: "estimated",
    note: "curvature of the efficiency parabola",
  },
  { section: "wastegate", key: "area_eff_m2", value: "2.00e-4", source: "estimated" },
  { section: "wastegate", key: "c_flow", value: "[0.6666, 5.3517]", source: "estimated" },
  { section: "turbocharger", key: "inertia_kg_m2", value: "5.0e-5", source: "estimated" },
  {
    section: "turbocharger",
    key: "omega_max",
    value: "18640.0",
    source: "published",
    note: "178,000 rpm demonstrated containment",
  },
  {
    section: "turbocharger",
    key: "omega_min",
    value: "300.0",
    source: "estimated",
    note: "floor so the shaft power balance stays finite",
  },
  {
    section: "control",
    key: "map_setpoint_pa",
    value: "3.10e5",
    source: "estimated",
    note: "at full fuelling",
  },
  {
    section: "control",
    key: "map_setpoint_zero_pa",
    value: "0.20e5",
    source: "estimated",
    note: "the schedule extrapolated to zero fuelling",
  },
  { section: "control", key: "kp", value: "8.0e-6", source: "estimated" },
  { section: "control", key: "ki", value: "2.0e-6", source: "estimated" },
  {
    section: "control",
    key: "integral_limit",
    value: "5.0e6",
    source: "estimated",
    note: "anti-windup clamp",
  },
  {
    section: "control",
    key: "turbo_omega_limit",
    value: "17700.0",
    source: "estimated",
    note: "169,000 rpm",
  },
  { section: "control", key: "kp_overspeed", value: "5.0e-3", source: "estimated" },
  { section: "thermal", key: "heat_fraction_to_head", value: "0.25", source: "estimated" },
  { section: "thermal", key: "head_conductance_w_per_k", value: "199.0", source: "estimated" },
  { section: "thermal", key: "head_capacity_j_per_k", value: "2700.0", source: "estimated" },
  { section: "cooling", key: "radiator_area_m2", value: "0.110", source: "estimated" },
  { section: "cooling", key: "radiator_effectiveness", value: "0.70", source: "estimated" },
  {
    section: "cooling",
    key: "air_flow_constant",
    value: "0.50",
    source: "published",
    note: "range, value estimated within it",
  },
  { section: "cooling", key: "coolant_capacity_j_per_k", value: "60000.0", source: "estimated" },
  { section: "cooling", key: "thermostat_open_k", value: "356.0", source: "estimated" },
  { section: "cooling", key: "thermostat_band_k", value: "12.0", source: "estimated" },
  {
    section: "cooling",
    key: "bypass_fraction",
    value: "0.05",
    source: "estimated",
    note: "the radiator is never fully isolated",
  },
  { section: "oil", key: "vogel", value: "[1.545e-4, 985.0, 160.0]", source: "estimated" },
  { section: "oil", key: "pressure_coefficient", value: "48234.0", source: "estimated" },
  { section: "oil", key: "relief_pressure_pa", value: "5.5e5", source: "estimated" },
  { section: "oil", key: "heat_fraction_from_fuel", value: "0.04", source: "estimated" },
  {
    section: "oil",
    key: "capacity_j_per_k",
    value: "15000.0",
    source: "estimated",
    note: "about 6 litres plus the sump",
  },
  { section: "oil", key: "cooler_area_m2", value: "0.028", source: "estimated" },
  { section: "oil", key: "cooler_effectiveness", value: "0.65", source: "estimated" },
  { section: "oil", key: "thermostat_open_k", value: "350.0", source: "estimated" },
  { section: "oil", key: "thermostat_band_k", value: "12.0", source: "estimated" },
  { section: "oil", key: "bypass_fraction", value: "0.08", source: "estimated" },
  {
    section: "limits",
    key: "rpm_max",
    value: "4220.0",
    source: "published",
    note: "maximum engine overspeed",
  },
  {
    section: "limits",
    key: "lambda_min",
    value: "1.30",
    source: "estimated",
    note: "smoke limit for a modern common-rail diesel",
  },
  {
    section: "limits",
    key: "rated_power_w",
    value: "132000.0",
    source: "published",
    note: "take-off rating at 3880 rpm",
  },
  {
    section: "limits.redline",
    key: "oil_t_max_k",
    value: "412.15",
    source: "published",
    note: "139 C",
  },
  {
    section: "limits.redline",
    key: "oil_p_min_pa",
    value: "2.5e5",
    source: "published",
    note: "2.5 bar at maximum continuous",
  },
  {
    section: "limits.redline",
    key: "oil_p_max_pa",
    value: "6.5e5",
    source: "published",
    note: "6.5 bar",
  },
  {
    section: "limits.redline",
    key: "coolant_t_max_k",
    value: "373.15",
    source: "published",
    note: "100 C",
  },
  {
    section: "limits.redline",
    key: "egt_max_k",
    value: "1123.15",
    source: "estimated",
    note: "850 C, the top of the band a turbo diesel runs at",
  },
  {
    section: "limits.redline",
    key: "cht_max_k",
    value: "523.15",
    source: "estimated",
    note: "250 C at the head metal",
  },
];

export const PUBLISHED_COUNT = PARAMETERS.filter((p) => p.source === "published").length;
export const ESTIMATED_COUNT = PARAMETERS.filter((p) => p.source === "estimated").length;

/**
 * Sections in the order the parameter file declares them, which is the order the
 * model consumes them: geometry, then the working fluids, then the cylinder, then
 * everything hung off it.
 */
export const SECTIONS = [...new Set(PARAMETERS.map((p) => p.section))];
