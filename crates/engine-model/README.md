# engine-model

Mean value engine model of a turbocharged heavy-fuel aero piston engine: ISA atmosphere, manifold filling, compressible restriction flow, volumetric efficiency, fuelling with a smoke limit, indicated work, friction and pumping losses, turbocharger shaft dynamics, wastegate control, per-cylinder combustion and thermal nodes.

The engine is compression ignition, not spark ignition. There is no throttle plate, load is set by injected fuel quantity, and the excess air ratio is an output rather than a controlled variable.

Pure. No I/O, no async, no clock. The caller owns the integration loop, which is what allows a thirty-hour mission to be projected in seconds.

Engine parameters live in TOML and every constant is annotated `published` or `estimated`.

```
cargo run -p engine-model --example power_sweep   # steady power and torque vs speed
cargo run -p engine-model --example load_step     # fuelling step transient
```
