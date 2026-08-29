# RQ-4 DRAGONFLY

Real-time digital twin and health-monitoring ground control station for the aero piston engine of a MALE UAV.

Conventional engine monitoring is threshold-based: a light comes on once oil pressure has already dropped below a limit. On a 30-hour ISR mission that is too late. DRAGONFLY runs a physics-accurate virtual engine in lockstep with the real one and watches the _residual_, the disagreement between measurement and model. A fault shows up in the residual while every gauge is still green.
