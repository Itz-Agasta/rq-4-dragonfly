# Fault signature matrix

This page lists the residual pattern each fault produces, which is what the ground station matches against to name a fault. `just signatures` generates it: each row comes from running the engine model with one parameter perturbed, settling it, and differencing the result against the healthy engine at the same operating point. Don't edit it by hand.

Engine: Austro Engine E4P (AE330), 180 hp heavy-fuel aero diesel

The reference point is cruise: 3720 rpm at 22,400 ft on a standard day. Each row is a unit vector, the direction a fault pushes the residual, measured in each channel's own standard deviations with severity divided out. A row with one large cell is a single-channel fault, and a row spread across many cells isn't.

Channel names: MAP is manifold absolute pressure, MAT manifold air temperature, MAF mass air flow, EGT exhaust gas temperature, CHT cylinder head temperature, and LAMBDA the excess air ratio. A dot means the component is below 0.01.

| hypothesis | RPM | MAP | MAT | MAF | TURBO | TORQUE | FUEL | OIL P | OIL T | COOLANT | EGT 1 | EGT 2 | EGT 3 | EGT 4 | CHT 1 | CHT 2 | CHT 3 | CHT 4 | LAMBDA 1 | LAMBDA 2 | LAMBDA 3 | LAMBDA 4 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| INJECTOR 3 COKING | . | . | . | . | . | -0.31 | -0.35 | +0.01 | . | . | . | . | -0.64 | . | . | . | -0.23 | . | . | . | +0.57 | . |
| CYLINDER 3 MISFIRE | . | . | . | . | . | -0.33 | . | +0.01 | . | . | . | . | -0.67 | . | . | . | -0.23 | . | . | . | +0.62 | . |
| EGT 3 SENSOR DRIFT | . | . | . | . | . | . | . | . | . | . | . | . | +1.00 | . | . | . | . | . | . | . | . | . |
| RADIATOR FOULING | . | . | . | . | . | . | . | . | . | +0.55 | . | . | . | . | +0.42 | +0.42 | +0.42 | +0.42 | . | . | . | . |
| COMPRESSOR EROSION | . | . | +0.24 | -0.22 | -0.10 | -0.19 | . | . | . | . | +0.45 | +0.45 | +0.45 | +0.45 | . | . | . | . | -0.09 | -0.09 | -0.09 | -0.09 |
| TURBINE EROSION | . | . | . | . | . | -0.28 | . | . | . | . | +0.48 | +0.48 | +0.48 | +0.48 | . | . | . | . | . | . | . | . |
| OIL SUPPLY LOSS | . | . | . | . | . | . | . | -1.00 | . | . | . | . | . | . | . | . | . | . | . | . | . | . |
| INTAKE RESTRICTION | . | . | . | -0.67 | -0.30 | -0.02 | . | . | . | . | +0.20 | +0.20 | +0.20 | +0.20 | . | . | . | . | -0.27 | -0.27 | -0.27 | -0.27 |

## How well each pair of faults can be told apart

Each cell is the cosine between two signatures. A value of 1 means the residual pattern alone can't separate the two faults, so something outside this matrix has to. A value of 0 means they're orthogonal, and any observation decides between them.

| | I3C | C3M | E3SD | RF | CE | TE | OSL | IR |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| I3C | +1.00 | +0.94 | -0.64 | -0.10 | -0.27 | -0.21 | -0.01 | -0.27 |
| C3M | +0.94 | +1.00 | -0.67 | -0.10 | -0.29 | -0.22 | -0.01 | -0.29 |
| E3SD | -0.64 | -0.67 | +1.00 | +0.00 | +0.45 | +0.48 | +0.00 | +0.20 |
| RF | -0.10 | -0.10 | +0.00 | +1.00 | -0.00 | -0.00 | +0.00 | +0.00 |
| CE | -0.27 | -0.29 | +0.45 | -0.00 | +1.00 | +0.92 | +0.00 | +0.63 |
| TE | -0.21 | -0.22 | +0.48 | -0.00 | +0.92 | +1.00 | +0.00 | +0.38 |
| OSL | -0.01 | -0.01 | +0.00 | +0.00 | +0.00 | +0.00 | +1.00 | -0.00 |
| IR | -0.27 | -0.29 | +0.20 | +0.00 | +0.63 | +0.38 | -0.00 | +1.00 |
