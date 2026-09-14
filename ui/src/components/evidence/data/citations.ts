/**
 * Every paper, specification and certificate the build rests on, with the file
 * that cites it.
 *
 * Transcribed from the `///` and `//!` blocks in `crates/`, where each of these
 * sits above the code it underwrites. The `cited` field is the check: a reader
 * who doubts a row opens that file and finds the same reference over the same
 * equation. Nothing here was collected for the display; the display collects
 * what the code already carried.
 *
 * MOCK: hand-transcribed rather than extracted at build time, because this
 * screen is scheduled to move to the landing page's /docs and a generator that
 * outlives it would be the thing kept in step for nothing.
 */

export interface Citation {
  /** Authors, surname order as the citing comment gives them. */
  authors: string;
  /** Verbatim. Omitted where the citing comment gives none and none was verified. */
  title?: string;
  /** Journal or proceedings and year. */
  venue: string;
  /** DOI, specification or archive link. Pinned, never a bare `main`. Omitted for a book. */
  href?: string;
  /** What the paper is load bearing for here. */
  uses: string;
  /** Paths, relative to the repository root, that carry this reference. */
  cited: string[];
}

export interface CitationGroup {
  title: string;
  /** One line on what this group of sources is answerable for. */
  note: string;
  papers: Citation[];
}

export const LITERATURE: CitationGroup[] = [
  {
    title: "Mean value engine model",
    note: "The model class, and the two papers the state equations come from.",
    papers: [
      {
        authors: "Wahlstrom & Eriksson",
        title:
          "Modelling diesel engines with a variable-geometry turbocharger and exhaust gas recirculation by optimization of model parameters for capturing non-linear system dynamics",
        venue: "Proc IMechE Part D 225(7), 2011",
        href: "https://doi.org/10.1177/0954407011398177",
        uses: "Model structure, volumetric efficiency form, manifold filling treatment, gas properties table 6.",
        cited: ["crates/engine-model/src/lib.rs", "crates/engine-model/src/cylinder.rs"],
      },
      {
        authors: "Ekberg, Leek & Eriksson",
        title: "Validation of an Open-Source Mean-Value Heavy-Duty Diesel Engine Model",
        venue: "SIMS 59, 2018",
        href: "https://doi.org/10.3384/ecp18153290",
        uses: "Torque decomposition and the cylinder-out gas temperature, eq. 17d and eq. 21.",
        cited: ["crates/engine-model/src/lib.rs", "crates/engine-model/src/cylinder.rs"],
      },
      {
        authors: "Sivertsson & Eriksson",
        title: "Modelling for Optimal Control: A Validated Diesel-Electric Powertrain Model",
        venue: "SIMS 55, 2014, pp. 49-58",
        href: "https://ep.liu.se/ecp/108/005/ecp14108005.pdf",
        uses: "Combustion efficiency island in equivalence ratio, and the turbocharger inertia scaling by displacement.",
        cited: [
          "crates/engine-model/src/compressor.rs",
          "crates/engine-model/src/engines/ae330.toml",
        ],
      },
      {
        authors: "Heywood",
        title: "Internal Combustion Engine Fundamentals, ch. 13",
        venue: "McGraw-Hill, 1988",
        uses: "Friction mean effective pressure as a quadratic in crank speed, the classical form.",
        cited: ["crates/engine-model/src/engines/ae330.toml"],
      },
    ],
  },
  {
    title: "Turbomachinery",
    note: "The compressor is where an aero piston twin is easiest to get wrong, because the map is the part nobody publishes.",
    papers: [
      {
        authors: "Leufven & Eriksson",
        title: "A surge and choke capable compressor flow model for control purposes",
        venue: "Control Engineering Practice 21(12), 2013",
        href: "https://doi.org/10.1016/j.conengprac.2013.08.006",
        uses: "The ellipse model: pressure ratio and corrected mass flow without a measured map.",
        cited: ["crates/engine-model/src/compressor.rs"],
      },
      {
        authors: "Mishra & Saad",
        title:
          "Simulation based study on improving the transient response quality of turbocharged diesel engines",
        venue: "Journal of Quality in Maintenance Engineering 23(3), 2017",
        href: "https://doi.org/10.1108/JQME-08-2016-0037",
        uses: "Models its compressor wheel at 6e-5 kg m2, the upper bound the shipped 5.0e-5 sits under.",
        cited: ["crates/engine-model/src/engines/ae330.toml"],
      },
    ],
  },
  {
    title: "Thermal and cooling",
    note: "Why hot, low and slow is the binding case and high altitude is not.",
    papers: [
      {
        authors: "Chalet, Lesage, Cormerais & Marimbordes",
        title: "Nodal modelling for advanced thermal-management of internal combustion engine",
        venue: "Applied Energy 190, 2017",
        href: "https://doi.org/10.1016/j.apenergy.2016.12.115",
        uses: "The lumped head, coolant and oil node structure and its conductances.",
        cited: ["crates/engine-model/src/thermal.rs"],
      },
      {
        authors: "Eriksson",
        title: "Mean value models for exhaust system temperatures",
        venue: "SAE 2002-01-0374",
        href: "https://doi.org/10.4271/2002-01-0374",
        uses: "Exhaust manifold heat loss as one identifiable coefficient-area product.",
        cited: ["crates/engine-model/src/engines/ae330.toml"],
      },
      {
        authors: "Parsons & Harper",
        title: "Radiators for aircraft engines",
        venue: "NBS Technologic Paper 211, 1922, tab. 21",
        href: "https://nvlpubs.nist.gov/nistpubs/nbstechnologic/nbstechnologicpaperT211.pdf",
        uses: "Air flow constant, the fraction of the approaching stream that passes the core rather than spilling around it: 0.4 to 0.7 over about 80 cores, as low as 0.3. The shipped value is 0.50.",
        cited: ["crates/engine-model/src/engines/ae330.toml"],
      },
      {
        authors: "Sharma",
        title: "Surface Temperature Prediction and Thermal Analysis of Cylinder Head",
        venue: "IJERA 3(4), fig. 12",
        href: "https://www.ijera.com/papers/Vol3_issue4/ET34892902.pdf",
        uses: "Measured head metal to 558 K at the exhaust valve bridge, which is why the CHT limit is 523.15 K and not 453.",
        cited: ["crates/engine-model/src/engines/ae330.toml"],
      },
    ],
  },
  {
    title: "State estimation",
    note: "A 28-state joint state-and-parameter filter, and the constraint handling that keeps it from proposing a negative discharge coefficient.",
    papers: [
      {
        authors: "Julier & Uhlmann",
        title: "Unscented Filtering and Nonlinear Estimation",
        venue: "Proceedings of the IEEE 92(3), 2004",
        href: "https://doi.org/10.1109/JPROC.2003.823141",
        uses: "The scaled unscented transform and the weight derivation.",
        cited: ["crates/twin-core/src/ukf.rs"],
      },
      {
        authors: "van der Merwe & Wan",
        title: "The square-root unscented Kalman filter for state and parameter-estimation",
        venue: "ICASSP 2001",
        href: "https://doi.org/10.1109/ICASSP.2001.940586",
        uses: "The joint state-and-parameter formulation the augmented state implements.",
        cited: ["crates/twin-core/src/ukf.rs"],
      },
      {
        authors: "Borguet, Dewallef & Leonard",
        title:
          "A Way to Deal With Model-Plant Mismatch for a Reliable Diagnosis in Transient Operation",
        venue: "Journal of Engineering for Gas Turbines and Power 130(3), 2008",
        href: "https://doi.org/10.1115/1.2833491",
        uses: "Covariance inflation during a transient, so a fuelling step does not read as a fault.",
        cited: ["crates/twin-core/src/twin.rs"],
      },
      {
        authors: "Assfalg, Allgower & Fritz",
        title:
          "Constrained derivative-free augmented state estimation for a diesel engine air path",
        venue: "IFAC Proceedings 39(2), 2006",
        href: "https://doi.org/10.3182/20060329-3-au-2901.00224",
        uses: "Parameter bounds applied to the estimate and not to the covariance.",
        cited: ["crates/twin-core/src/twin.rs"],
      },
      {
        authors: "Simon",
        title:
          "Kalman filtering with state constraints: a survey of linear and nonlinear algorithms, sec. 4",
        venue: "IET Control Theory & Applications 4(8), 2010",
        href: "https://doi.org/10.1049/iet-cta.2009.0032",
        uses: "Clamping every sigma point, not only the mean, before it enters a model that rejects a negative bound.",
        cited: ["crates/twin-core/src/health.rs"],
      },
      {
        authors: "George & Muthuveerappan",
        title:
          "Determination of Time Constant of Temperature Sensors and its Application in Aero Gas Turbine Engines",
        venue:
          "Journal of Aerospace Sciences and Technologies 71(4), 2023, pp. 378-385. DRDO Gas Turbine Research Establishment",
        href: "https://doi.org/10.61653/joast.v71i4.2019.174",
        uses: "Measures the same 1.6 mm type K probe at 21 s on a bench and 7 s in an engine, where gas velocity lifts the film coefficient. The measurement source and the filter must agree on this or every transient reads as a fault.",
        cited: ["crates/twin-core/src/twin.rs", "crates/dragonfly-sim/src/sensors.rs"],
      },
    ],
  },
  {
    title: "Detection",
    note: "Both detectors are classical, published and cheap enough to run at 20 Hz on an edge node.",
    papers: [
      {
        authors: "Page",
        title: "Continuous Inspection Schemes",
        venue: "Biometrika 41(1/2), 1954",
        href: "https://doi.org/10.1093/biomet/41.1-2.100",
        uses: "CUSUM. Slack sits at half the shift to be noticed, which gives an in-control run length near 465 samples per channel.",
        cited: ["crates/twin-core/src/detect.rs"],
      },
      {
        authors: "Wilson & Hilferty",
        title: "The Distribution of Chi-Square",
        venue: "PNAS 17(12), 1931",
        href: "https://doi.org/10.1073/pnas.17.12.684",
        uses: "Chi-square upper quantile for the whole-vector test, better than half a percent above k = 10, and no statistics dependency.",
        cited: ["crates/twin-core/src/detect.rs"],
      },
      {
        authors: "Acklam",
        title: "An algorithm for computing the inverse normal cumulative distribution function",
        venue: "archived note, 2010",
        href: "https://web.archive.org/web/20151030215612/http://home.online.no/~pjacklam/notes/invnorm/",
        uses: "Both branches of the standard normal quantile; the tail branch is required at p = 0.999.",
        cited: ["crates/twin-core/src/detect.rs"],
      },
    ],
  },
  {
    title: "Prognosis",
    note: "Why the remaining life carries an interval, and why that interval is labelled covariance-derived rather than conformal.",
    papers: [
      {
        authors: "Celaya, Saxena & Goebel",
        title:
          "Uncertainty Representation and Interpretation in Model-based Prognostics Algorithms based on Kalman Filter Estimation",
        venue: "Annual Conference of the PHM Society, 2012",
        href: "https://doi.org/10.36001/phmconf.2012.v4i1.2110",
        uses: "Forecast-to-threshold formulation, and the warning that a remaining life is a ratio and is not normally distributed.",
        cited: ["crates/prognostics/src/rul.rs"],
      },
      {
        authors: "Keizers, Loendersloot & Tinga",
        title:
          "Unscented Kalman Filtering for Prognostics Under Varying Operational and Environmental Conditions",
        venue: "International Journal of Prognostics and Health Management 12(2), 2021",
        href: "https://doi.org/10.36001/ijphm.2021.v12i2.2943",
        uses: "Per-parameter process noise scaled to each degradation's own timescale, and the trend extrapolation being a different estimation problem from tracking.",
        cited: ["crates/twin-core/src/health.rs", "crates/prognostics/src/trend.rs"],
      },
    ],
  },
  {
    title: "Fault physics",
    note: "Every injected fault is rated against a measurement in the literature, so a demonstration cannot quietly run a fault ten times faster than the real one.",
    papers: [
      {
        authors: "Payri, Salvador, Carreres & De la Morena",
        title:
          "Fuel temperature influence on the performance of a last generation common-rail diesel ballistic injector. Part II: 1D model development, validation and analysis",
        venue: "Energy Conversion and Management 114, 2016, pp. 376-391",
        href: "https://doi.org/10.1016/j.enconman.2016.02.043",
        uses: "Nominal injector discharge coefficient near 0.96 for hydroground conical nozzles, against 0.86 for a cavitating cylindrical one.",
        cited: ["crates/twin-core/src/health.rs"],
      },
      {
        authors: "Sheykhvazayefi et al.",
        venue: "International Journal of Automotive Engineering 13(4)",
        href: "https://doi.org/10.20485/jsaeijae.13.4_177",
        uses: "75 to 90% hole blockage measured only at 800 to 900 h, which is why the accelerated coking mission is a claim about the estimator and never about a real failure rate.",
        cited: ["crates/mission-gen/tests/prognosis.rs"],
      },
      {
        authors: "Tamura, Saito, Murata, Kokubu & Morimoto",
        title:
          "Misfire detection of internal combustion engines using wave form of exhaust gas temperature",
        venue: "Trans. JSME C 77(780), 2011",
        href: "https://doi.org/10.1299/kikaic.77.3094",
        uses: "The exhaust temperature signature of a misfiring cylinder, the worked case in sec. 2.1.",
        cited: ["crates/dragonfly-sim/src/fault.rs"],
      },
      {
        authors: "Storey, Sluder, Lance et al.",
        title: "Exhaust gas recirculation cooler fouling in diesel applications, tab. 1",
        venue: "Heat Exchanger Fouling and Cleaning XI, 2015",
        href: "https://heatexchanger-fouling.com/wp-content/uploads/2021/09/11_Storey_F.pdf",
        uses: "Effectiveness loss plateaus at 16.7% and no anti-stick coating changed it, which is why radiator fouling is modelled as asymptotic and not linear.",
        cited: ["crates/dragonfly-sim/src/fault.rs"],
      },
      {
        authors: "Tucker, Edler, Zuzek et al.",
        title: "Thermoelectric stability of dual-wall and conventional type K and N thermocouples",
        venue: "Measurement Science and Technology 33, 2022",
        href: "https://doi.org/10.1088/1361-6501/ac57ee",
        uses: "Aged type K drifts about -0.005 K/h, so anything faster is a signal-chain fault and not probe oxidation. The default drift is labelled accordingly.",
        cited: ["crates/dragonfly-sim/src/fault.rs"],
      },
      {
        authors: "Gao, Zhang, Cao et al.",
        title: "Simulation study on multi-mode misfire fault of diesel engine",
        venue: "IET conference proceedings, 2025",
        href: "https://doi.org/10.1049/icp.2025.3373",
        uses: "A single-cylinder misfire is localised from the amplitude and phase of the half engine order alone, which is the feature the synthesised vibration channel carries.",
        cited: ["crates/dragonfly-sim/src/sensors.rs"],
      },
    ],
  },
  {
    title: "Bus and codec",
    note: "The wire format is not ours to invent. Every field is the published definition, and the signatures are checked against the reference implementation.",
    papers: [
      {
        authors: "DroneCAN",
        title: "Specification, sections 3 and 4.1",
        venue: "dronecan.github.io",
        href: "https://dronecan.github.io/Specification/",
        uses: "DSDL bit layout, saturated field semantics, CRC-16-CCITT-FALSE payload check and CRC-64-WE type signature.",
        cited: [
          "crates/dronecan-ice/src/bits.rs",
          "crates/dronecan-ice/src/crc.rs",
          "crates/dronecan-ice/src/transfer.rs",
        ],
      },
      {
        authors: "DroneCAN DSDL",
        title: "uavcan.equipment.ice.reciprocating.Status and the equipment tree",
        venue: "github.com/dronecan/DSDL",
        href: "https://github.com/dronecan/DSDL/tree/master/uavcan/equipment",
        uses: "The seven message definitions, field by field. Semantics cross-checked against PX4 InternalCombustionEngineStatus and ArduPilot EFI/EFI2/ECYL.",
        cited: ["crates/dronecan-ice/src/messages/"],
      },
      {
        authors: "OpenCyphal forum",
        title: "Data type signature reference table",
        venue: "forum.opencyphal.org",
        href: "https://forum.opencyphal.org/t/data-type-signature/241",
        uses: "Seven published signatures the compile-time hash is asserted against, which is the licence to trust the eighth, the vendor-specific one.",
        cited: ["crates/dronecan-ice/src/signature.rs"],
      },
    ],
  },
  {
    title: "Certification",
    note: "The only document in this list that is not a paper, and the one every published engine number traces to.",
    papers: [
      {
        authors: "EASA",
        title: "Type Certificate Data Sheet E.200, Austro Engine E4 series",
        venue: "European Union Aviation Safety Agency",
        href: "https://www.easa.europa.eu/en/downloads/7617/en",
        uses: "Ratings, speeds, the propeller shaft torque limit, the operating limits the redlines are taken from, and the certified ceiling.",
        cited: ["crates/engine-model/src/published.rs"],
      },
    ],
  },
];

/** Rows in the table above, stated rather than asserted. */
export const PAPER_COUNT = LITERATURE.reduce((n, g) => n + g.papers.length, 0);
