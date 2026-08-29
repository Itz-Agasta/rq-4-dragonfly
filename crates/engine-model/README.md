# engine-model

Mean value engine model of a turbocharged heavy-fuel aero piston engine: ISA atmosphere, manifold filling, compressible throttle flow, volumetric efficiency, torque and friction, turbocharger shaft dynamics, wastegate control, per-cylinder combustion and thermal nodes.

Pure. No I/O, no async, no clock. The caller owns the integration loop, which is what allows a 30-hour mission to be projected at 500x realtime.
