/**
 * Where every number on every other screen comes from, and how it is checked.
 *
 * The pipeline stages are transcribed from the crates they name. The counts are
 * the constants those crates declare: `twin.rs` DIM is 28, `channels.rs` CHANNELS
 * is 22, `indices.rs` INDICES is 7, `health.rs` PARAMS is 10, `integrator.rs` DT
 * is 1/200. None of them is a round number chosen for a slide.
 */

export interface Stage {
  /** Short label for the flow diagram. */
  id: string;
  title: string;
  /** Crate or file that is this stage. */
  owner: string;
  rate: string;
  detail: string;
}

export const PIPELINE: Stage[] = [
  {
    id: "engine",
    title: "Engine and sensors",
    owner: "dragonfly-sim",
    rate: "200 Hz",
    detail:
      "A mean value engine model integrated with fixed-step RK4, plus sensor lags, quantisation and noise. In a deployment this stage is the engine itself and this crate is deleted. Nothing downstream knows the difference, which is what keeps the rest of the stack honest.",
  },
  {
    id: "bus",
    title: "DroneCAN on SocketCAN",
    owner: "dronecan-ice",
    rate: "20 Hz, 3 nodes",
    detail:
      "Seven message types encoded to the published Data Structure Description Language (DSDL) bit layout, on a real CAN interface. The frames exist on the wire and can be captured with a standard CAN monitor. A dropped interface becomes a failure the ingest loop has to notice, which a function call would never produce.",
  },
  {
    id: "ingest",
    title: "Ingest and reassembly",
    owner: "dragonfly-core",
    rate: "per transfer",
    detail:
      "Multi-frame transfer reassembly with a cyclic redundancy check, per-node source ages and a link state. A frozen trace looks exactly like a steady one, so every value is drawn with its age. Anything older than 250 ms is stale at both ends.",
  },
  {
    id: "twin",
    title: "28-state UKF",
    owner: "twin-core",
    rate: "20 Hz, 1.30 ms",
    detail:
      "Six engine states, twelve per-cylinder states and ten health parameters, estimated together by an augmented-state unscented filter. Measured at 1.30 ms per frame in release: 2.6% of a core at 20 Hz.",
  },
  {
    id: "residual",
    title: "Residual against a healthy engine",
    owner: "twin-core/nominal.rs",
    rate: "22 channels",
    detail:
      "The one that matters. An estimator that also tracks health parameters drives its own innovation to zero whether or not the machine is sick. Feed that innovation to a display and a coked injector shows up as nothing at all. Every display and every detector here reads the residual against a nominal engine instead.",
  },
  {
    id: "detect",
    title: "Detection and isolation",
    owner: "twin-core/detect.rs",
    rate: "per channel",
    detail:
      "A cumulative sum (CUSUM) test per channel and a chi-square test on the whole vector, then the residual is projected onto a generated signature matrix to rank eight hypotheses. Both detectors are 1950s statistics, which is why an engineer can audit them by hand and why they add microseconds to a frame.",
  },
  {
    id: "health",
    title: "Seven subsystem indices",
    owner: "twin-core/indices.rs",
    rate: "20 Hz",
    detail:
      "Ten health parameters scored into seven subsystem indices. Compressor and turbine share one index. The shaft power balance responds to their product and ignores their ratio, so scoring them apart would invent a number the data cannot support.",
  },
  {
    id: "rul",
    title: "Trend and remaining life",
    owner: "prognostics",
    rate: "on new estimate",
    detail:
      "A trend fitted to each health parameter, then extrapolated to its functional-failure threshold. The interval is covariance-derived and labelled that way on screen. It is not conformal, because conformal coverage has to be earned from a calibration set of run-to-failure trajectories.",
  },
  {
    id: "ui",
    title: "Screens",
    owner: "ui/",
    rate: "20 Hz in, 5 Hz read",
    detail:
      "One WebSocket, one render loop, no telemetry in React state. Readouts update at 5 Hz, because a digit that changes twenty times a second cannot be read. Bars tween between those updates so the geometry stays continuous.",
  },
];

export interface Check {
  claim: string;
  how: string;
}

/** What is actually checked, and by what. Each row names a command a reader can run. */
export const VERIFICATION: Check[] = [
  {
    claim: "The model reproduces the certificated rating point",
    how: "A sweep across the certified envelope regenerates the validation document and every figure in it. No number in that document is authored, so changing one means changing the model.",
  },
  {
    claim: "The parameter file and the certificate have not drifted apart",
    how: "`published.rs` holds the certificated figures apart from the parameters the model runs on, and tests assert the two against each other. One test checks that bore and stroke reproduce the stated displacement. Another checks that published power and torque agree through the gearbox.",
  },
  {
    claim: "The E4P has not been confused with the E4",
    how: 'A `const` block fails the build if the shipped rating ever drops to the sibling variant\'s. An edit that "corrects" this model to 168 hp does not compile.',
  },
  {
    claim: "The wire format is the published one",
    how: "Test vectors are generated by pydronecan, the reference implementation, and the codec is asserted against them byte for byte. Seven type signatures are computed at compile time and checked against the published table. Agreement on those seven is what makes the eighth trustworthy, since no published value exists for it.",
  },
  {
    claim: "The fault signatures are derived, not drawn",
    how: "The matrix is derived by perturbing one parameter at a time, settling the model, and differencing the result against the healthy engine at the same operating point. Separability cosines between every pair are published alongside it.",
  },
  {
    claim: "The detectors survive a real bus",
    how: "Measured on a live CAN interface at real time, not asserted in a unit test. A passing suite has twice shipped a detector that failed within seconds of a live run, so a detection claim names the profile it was measured on.",
  },
];

/**
 * The dataset question, answered.
 *
 * This is the paragraph a reviewer is owed and it is the one most likely to be
 * fudged elsewhere, so it states what does not exist as plainly as what does.
 */
export const DATA_ANSWER = {
  question: "Which dataset was this trained on?",
  headline: "None. The reference is physics, not a training set.",
  body: [
    "No public run-to-failure dataset exists for aero piston engines. Every candidate is the wrong machine. C-MAPSS and N-CMAPSS are turbofan, EngineAD is automotive, and the published unmanned aerial vehicle prognostics sets are electric multirotors. The gap belongs to the field. NASA built C-MAPSS in the first place because turbofan run-to-failure data was equally unavailable.",
    "Diagnosis here is model-based instead. A validated thermodynamic model runs alongside the real engine, and the system watches the residual: measurement minus physics. A coked injector is detected because the engine stops matching the physics, not because a coked injector appeared in training.",
    "That removes the training set from the argument. There is no distribution to drift away from, no under-represented class, and no way to be confidently wrong about a fault nobody thought to record. The system still needs a reference, and that reference is the published rating point, the certificated limits and the fault physics literature. All three are in the public record.",
  ],
  ownData: [
    [
      "Recorded missions",
      "Parquet, with the schema generated from the channel and parameter tables rather than written by hand. The pipeline runs offline on mission time at 34x real, so a four-hour flight takes about seven minutes to record. REPLAY reads these.",
    ],
    [
      "Generated fault signatures",
      "Eight hypotheses across 22 channels, each derived by perturbing one model parameter and measuring which channels move.",
    ],
    [
      "Golden bus vectors",
      "DroneCAN frames generated by pydronecan and committed, so the codec is checked against the reference implementation and not against itself.",
    ],
  ] as [string, string][],
  roadmap: [
    [
      "Conformal RUL intervals",
      "The shipped interval is covariance-derived and is labelled that way on screen. Conformal coverage requires a calibration set of run-to-failure trajectories, which the mission recorder can produce but which has not been generated at that scale.",
    ],
    [
      "Real telemetry cross-check",
      "ArduPilot `.bin` logs contain EFI2 and ECYL messages, and PX4 `.ulg` logs contain InternalCombustionEngineStatus. Field semantics are cross-checked against both definitions. The logs themselves have not been obtained, and no claim on this page rests on them.",
    ],
    [
      "Learned residual model",
      "An autoencoder over healthy residuals would give the cumulative sum detector a second opinion. It is deliberately absent from the runtime. Python stays out of the hot path, and an untrained model would add nothing but a dependency.",
    ],
  ] as [string, string][],
};

export interface Coverage {
  section: string;
  title: string;
  items: [string, string][];
}

/** Every bullet of the problem statement, against what answers it. */
export const PS_COVERAGE: Coverage[] = [
  {
    section: "A",
    title: "Digital twin core framework",
    items: [
      [
        "Virtual engine model synchronised with live data",
        "twin-core, 28-state UKF at 20 Hz. TWIN screen.",
      ],
      [
        "Modular architecture",
        "Seven crates. engine-model has zero I/O and zero async, which is what buys 500x projection.",
      ],
      ["Real-time ingestion", "SocketCAN, DroneCAN, three nodes. OPS screen."],
    ],
  },
  {
    section: "B",
    title: "Health monitoring",
    items: [
      ["RPM, CHT, EGT, oil, fuel flow", "22 channels, all measured against twin prediction. TWIN."],
      [
        "Vibration signatures",
        "Synthesised at half engine order from the misfire literature, and labelled as synthesised. A mean value model has no individual cycles to disturb.",
      ],
      [
        "Battery and alternator health",
        "Reported over uavcan.equipment.power.CircuitStatus. Monitored, not modelled.",
      ],
      [
        "Injection timing parameters",
        "Commanded injection duration is the fuelling input to the twin. Not throttle position, which quantises.",
      ],
      ["Health indices", "Seven subsystem indices from ten estimated parameters. OPS rail."],
    ],
  },
  {
    section: "C",
    title: "Fault detection and predictive analytics",
    items: [
      [
        "Misfire, injector abnormality, coking",
        "Injected, detected and isolated. Four of eight named faults are implemented end to end.",
      ],
      [
        "Lubrication issues",
        "Oil supply loss is in the signature catalogue and is orthogonal to every other hypothesis.",
      ],
      [
        "Sensor drift and failure",
        "The discriminator. A real fault moves a group of channels the way the physics couples them. A lying instrument moves one.",
      ],
      [
        "Overheating trends",
        "Radiator fouling, rated to the measured 16.7% effectiveness plateau.",
      ],
      [
        "Combustion instability, abnormal vibration",
        "Not implemented, and listed here for that reason.",
      ],
    ],
  },
  {
    section: "D",
    title: "AI/ML layer",
    items: [
      [
        "Anomaly detection",
        "A CUSUM per channel and a chi-square on the whole residual vector. You can audit both by hand, and they run in microseconds.",
      ],
      [
        "RUL estimation",
        "Trend fitted per health parameter, extrapolated to a functional-failure threshold, with a covariance-derived interval.",
      ],
      ["Trend analysis", "ANALYSIS screen, per parameter, over the mission."],
      [
        "Maintenance recommendations",
        "Advisory generated from the ranked hypothesis and its remaining life.",
      ],
      ["Learned model in the runtime", "None. The reference is physics, not a training corpus."],
    ],
  },
  {
    section: "E",
    title: "Simulation and replay",
    items: [
      ["Replay of historical mission data", "Parquet recordings, scrubbable. REPLAY screen."],
      [
        "Environmental simulation",
        "Profiles including hot weather and high altitude. SIMULATE screen.",
      ],
      [
        "High altitude, endurance, hot weather, throttle transients",
        "All four profiles project forward from the live twin at 500x.",
      ],
    ],
  },
  {
    section: "F",
    title: "Visualisation dashboard",
    items: [
      ["Real-time health status, fault alerts", "OPS."],
      ["Efficiency trends, maintenance advisory", "ANALYSIS."],
      ["Mission-wise health reports", "REPLAY, plus the fleet roster."],
      [
        "Fleet-level monitoring",
        "FLEET. The roster is a static set by decision, and the screen says so on itself.",
      ],
    ],
  },
];
