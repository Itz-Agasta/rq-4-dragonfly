//! The stand-in for a real engine: simulation plus fault injection onto a CAN bus.
//!
//! Runs as a separate process and writes real DroneCAN frames to `vcan0`, so
//! `dragonfly-core` reads the same bytes it would read from a FADEC. Swapping to
//! hardware is a change of interface name, and it means the CAN rig can be built in
//! parallel without touching this code.
//!
//! Faults are physically grounded parameter perturbations inside `engine-model`,
//! never signal-level hacks. That is what makes residual-based diagnosis actually
//! work instead of merely appearing to.

fn main() {
    todo!("D5: publish ice.reciprocating.Status to vcan0")
}
