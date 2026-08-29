//! Mean value engine model (MVEM) for a turbocharged heavy-fuel aero piston engine.
//!
//! Pure by construction: no I/O, no async, no clock, no global state. The caller
//! owns the integration loop and hands in state plus inputs. That is what lets the
//! twin replay a 30-hour mission at 500x realtime, and it is what makes every
//! equation testable against a hand-computed case in isolation.
//!
//! Model class and equations follow Eriksson & Nielsen, *Modeling and Control of
//! Engines and Drivelines*, Wiley 2014, section 8.9 (turbocharged SI engine).
#![forbid(unsafe_code)]
