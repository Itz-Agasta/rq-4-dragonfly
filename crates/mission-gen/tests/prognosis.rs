//! What a realistic degradation rate does to the remaining life estimate.
//!
//! `cargo test -p mission-gen --release -- --ignored`. Three minutes of wall clock
//! for two hours of flight, which is why it is not in the default suite.
//!
//! # The rate is from the literature, not chosen
//!
//! Butmarasri, Nagasawa & Kosaka measured injector discharge coefficient on a
//! common-rail nozzle: it falls sharply through the first five hours and then
//! flattens, losing 17.7% by ten hours on a zinc-accelerated test. That is the
//! shape `InjectorCoking::scale_at` already had, at a time constant of 5.6 h.
//!
//! **The depth is accelerated and the shape is not.** Functional failure at Cd
//! 0.62 is a 36% loss, which on unadditised service fuel takes hundreds of hours:
//! Sheykhvazayefi et al. found 75 to 90% hole blockage only at 800 to 900 h. So
//! this is a claim about the estimator, never that an injector fails in eight
//! hours.
//!
//! <https://doi.org/10.20485/jsaeijae.13.4_177>
use dragonfly_sim::fault::{Faults, InjectorCoking};
use dragonfly_sim::mission::Profile;
use mission_gen::{Config, generate};
use twin_core::health::{INJECTOR_CD_NOMINAL, index as th};

/// Cylinder the fault is put on, zero based.
const CYLINDER: usize = 2;
/// Seconds before deposits begin to matter.
const ONSET_S: f64 = 300.0;
/// Three time constants, so 60,000 s is a 5.6 h constant. See the module note.
const RAMP_S: f64 = 60_000.0;
/// Flow scale the nozzle settles at. Below the failure threshold, so the
/// crossing happens at a finite time and there is a truth to compare against.
const FINAL_SCALE: f64 = 0.55;
/// Discharge coefficient at which the injector no longer meets its duty.
/// Mirrors `twin_core::health::DESCRIPTORS`, asserted below rather than trusted.
const FAILURE_CD: f64 = 0.62;

fn coking() -> InjectorCoking {
    InjectorCoking {
        cylinder: CYLINDER,
        onset_s: ONSET_S,
        ramp_s: RAMP_S,
        final_scale: FINAL_SCALE,
    }
}

/// True discharge coefficient at a mission time, from the simulator's own curve.
///
/// Reads `scale_at` rather than reimplementing it: a test that rebuilds the
/// severity curve it is checking tests arithmetic instead of the fault path, and
/// the two drift apart with nothing failing.
fn true_cd(t_s: f64) -> f64 {
    INJECTOR_CD_NOMINAL * coking().scale_at(t_s)
}

/// Mission time at which the true coefficient reaches the failure threshold.
///
/// Bisected rather than inverted in closed form, so the shape of `scale_at` can
/// change without this needing to know about it.
fn true_failure_s() -> f64 {
    let (mut low, mut high) = (ONSET_S, ONSET_S + RAMP_S);
    for _ in 0..80 {
        let mid = f64::midpoint(low, high);
        if true_cd(mid) > FAILURE_CD {
            low = mid;
        } else {
            high = mid;
        }
    }
    f64::midpoint(low, high)
}

fn fly(hours: f64) -> mission_gen::Summary {
    generate(&Config {
        out: None,
        profile: Profile::Cruise,
        hours,
        seed: 0x5EED,
        aux_dtid: 20950,
        faults: Faults {
            injector: Some(coking()),
            ..Faults::default()
        },
    })
    .expect("the mission runs")
}

/// The threshold the projection aims at has to be the one this test measures
/// against, or the two disagree about what failure means and the comparison is
/// meaningless.
#[test]
fn the_failure_threshold_is_the_one_the_estimator_uses() {
    let descriptor = twin_core::health::DESCRIPTORS[th::INJECTOR + CYLINDER];
    assert!((descriptor.failure - FAILURE_CD).abs() < 1e-9);
}

/// The whole point of generating a mission offline: a decline of a realistic
/// shape, and an estimate of where the parameter is that is good to a fraction
/// of a percent.
#[test]
#[ignore = "runs two hours of flight, about three minutes of wall clock"]
fn the_twin_recovers_a_slowly_coking_injector() {
    let summary = fly(2.0);
    let i = th::INJECTOR + CYLINDER;
    let truth = true_cd(summary.t_s);
    let error_pct = (summary.theta[i] - truth).abs() / truth * 100.0;
    println!(
        "t={:.0}s  estimate={:.4}  truth={:.4}  error={error_pct:.2}%",
        summary.t_s, summary.theta[i], truth
    );
    assert!(summary.locked, "the twin has to be locked to be believed");
    assert!(error_pct < 1.0, "injector estimate is {error_pct:.2}% out");
}

/// **The remaining life is conservative, and it converges as failure approaches.**
///
/// A straight line through a self-limiting curve reaches the threshold too soon
/// early on, and being short is the safe direction. The interval is
/// covariance-derived, so it says how well the *line* is pinned and nothing about
/// whether a line was the right model, which is why at two hours it is narrow and
/// still excludes the truth. `handover.md` 6.5 has the two and four hour numbers.
#[test]
#[ignore = "runs two hours of flight, about three minutes of wall clock"]
fn the_remaining_life_is_conservative_on_a_decelerating_fault() {
    let summary = fly(2.0);
    let i = th::INJECTOR + CYLINDER;
    let reported = summary.rul_hours[i].expect("a declining injector has a remaining life");
    let truth_h = (true_failure_s() - summary.t_s) / 3600.0;
    let ratio = reported / truth_h;
    println!(
        "reported={reported:.2}h  p10={:?}  p90={:?}  truth={truth_h:.2}h  ratio={ratio:.2}",
        summary.rul_p10[i], summary.rul_p90[i]
    );
    assert!(
        (0.5..0.8).contains(&ratio),
        "remaining life {reported:.2} h against a true {truth_h:.2} h is a ratio of {ratio:.2}"
    );
    let p90 = summary.rul_p90[i].expect("a bounded decline");
    assert!(
        p90 < truth_h,
        "the interval covers the truth now; the fit model must have changed"
    );
}
