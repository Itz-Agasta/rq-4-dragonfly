# dragonfly-sim

The stand-in for a real engine. Runs `engine-model` with injectable faults and publishes real DroneCAN frames to `vcan0`.

Faults are physically grounded parameter perturbations, never signal-level hacks, which is what makes residual-based diagnosis downstream actually work.
