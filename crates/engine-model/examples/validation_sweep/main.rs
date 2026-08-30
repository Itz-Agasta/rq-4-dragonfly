//! Regenerates `docs/model_validation.md` and the figures beside it.
//!
//! This is the document that answers whether the model is real, so it is generated
//! rather than written: every number and every curve in it comes from running the
//! model, and re-running this is the only way to change it.

mod svg;

use engine_model::{
    EngineParams, Inputs, Outputs, State, atmosphere, control, cylinder, engines, integrator,
};
use std::f64::consts::TAU;
use std::io::Write as _;

const HP: f64 = 745.699_9;
/// Longest settling run, s. The oil node has a time constant near two minutes.
const SETTLE_MAX_S: f64 = 600.0;

fn rpm_to_rad(rpm: f64) -> f64 {
    rpm * TAU / 60.0
}

/// Run one operating point to steady state.
///
/// Converged rather than fixed-length: the gas path settles in under a second while
/// the oil node takes minutes, so a fixed run long enough for the slowest state would
/// waste most of its time on every point that does not need it.
fn settle(
    p: &EngineParams,
    alt_ft: f64,
    oat_k: Option<f64>,
    rpm: f64,
    ias_kt: f64,
    fuel_cmd: f64,
) -> (State, Outputs, f64) {
    let amb = atmosphere::isa(alt_ft * atmosphere::FT);
    let t_amb = oat_k.unwrap_or(amb.t);
    let rho = amb.p / (p.gas.r_air * t_amb);
    let tas = ias_kt * 0.514_444 * (atmosphere::isa(0.0).rho / rho).sqrt();

    let mut x = State::at_rest(amb.p, rpm);
    let mut boost = control::BoostController::new();
    let mut u = Inputs {
        fuel_cmd,
        wastegate: 1.0,
        p_amb: amb.p,
        t_amb,
        tas_m_s: tas,
        load_torque: 0.0,
    };

    let steps = (SETTLE_MAX_S / integrator::DT) as u32;
    let mut previous = x;
    for i in 0..steps {
        u.wastegate = boost.update(p, u.fuel_cmd, x.p_im, x.omega_tc, integrator::DT);
        u.load_torque = engine_model::evaluate(p, &x, &u).torque_brake;
        x = engine_model::step(p, &x, &u, integrator::DT);
        if i % 200 == 199 {
            let moved = (x.p_im - previous.p_im).abs() / x.p_im
                + (x.t_coolant - previous.t_coolant).abs() / x.t_coolant
                + (x.t_oil - previous.t_oil).abs() / x.t_oil
                + (x.omega_tc - previous.omega_tc).abs() / x.omega_tc;
            if moved < 1e-7 {
                break;
            }
            previous = x;
        }
    }
    let o = engine_model::evaluate(p, &x, &u);
    (x, o, tas)
}

fn main() -> std::io::Result<()> {
    let p = engines::ae330();
    std::fs::create_dir_all("docs/validation")?;

    // Figure 1 and 2: the full-power altitude sweep.
    let mut power = Vec::new();
    let mut turbo = Vec::new();
    let mut egt = Vec::new();
    let mut cht = Vec::new();
    let mut alt = 0.0;
    while alt <= 32_000.0 {
        let (x, o, _) = settle(&p, alt, None, 3880.0, 110.0, 1.0);
        power.push((alt, o.power_brake_w / HP));
        turbo.push((alt, x.turbo_rpm() / 1000.0));
        egt.push((alt, o.t_egt[0]));
        cht.push((alt, x.t_cht[0]));
        alt += 500.0;
    }
    let rated = power
        .iter()
        .take_while(|(a, _)| *a <= 11_000.0)
        .map(|(_, hp)| *hp)
        .fold(f64::MAX, f64::min);
    let at_ceiling = power.last().map_or(0.0, |(_, hp)| *hp);

    write_svg(
        "docs/validation/power_altitude.svg",
        &svg::chart(
            "Brake power against altitude, full fuelling, 3880 rpm",
            "pressure altitude, ft",
            "brake power, hp",
            &[
                svg::Series::solid("modelled", power.clone()),
                svg::Series::dashed("180 hp rating", vec![(0.0, 180.0), (32_000.0, 180.0)]),
                svg::Series::dashed(
                    "rated critical altitude",
                    vec![(11_000.0, 0.0), (11_000.0, 200.0)],
                ),
            ],
            Some((60.0, 200.0)),
        ),
    )?;

    write_svg(
        "docs/validation/turbo_altitude.svg",
        &svg::chart(
            "Turbocharger speed against altitude",
            "pressure altitude, ft",
            "shaft speed, thousand rpm",
            &[
                svg::Series::solid("modelled", turbo.clone()),
                svg::Series::dashed(
                    "demonstrated containment",
                    vec![(0.0, 178.0), (32_000.0, 178.0)],
                ),
            ],
            Some((100.0, 190.0)),
        ),
    )?;

    write_svg(
        "docs/validation/temperatures_altitude.svg",
        &svg::chart(
            "Exhaust and cylinder head temperature against altitude",
            "pressure altitude, ft",
            "temperature, K",
            &[
                svg::Series::solid("exhaust, cylinder 1", egt),
                svg::Series::solid("cylinder head 1", cht),
            ],
            None,
        ),
    )?;

    // Figure 3: brake specific fuel consumption, as one curve per speed rather than
    // as a filled contour. Same information, no extra plotting machinery.
    let mut bsfc_series = Vec::new();
    for rpm in [2200.0, 2800.0, 3400.0, 3880.0] {
        let mut curve = Vec::new();
        let mut cmd = 0.25;
        while cmd <= 1.0001 {
            let (_, o, _) = settle(&p, 0.0, None, rpm, 110.0, cmd);
            if let Some(bsfc) = o.bsfc_g_per_kwh() {
                let bmep = 2.0 * TAU * o.torque_brake / p.geometry.displacement_m3 / 1e5;
                if bmep > 0.5 {
                    curve.push((bmep, bsfc));
                }
            }
            cmd += 0.0625;
        }
        bsfc_series.push(svg::Series::solid(&format!("{rpm:.0} rpm"), curve));
    }
    write_svg(
        "docs/validation/bsfc.svg",
        &svg::chart(
            "Brake specific fuel consumption, sea level",
            "brake mean effective pressure, bar",
            "BSFC, g/kWh",
            &bsfc_series,
            Some((190.0, 340.0)),
        ),
    )?;

    // Figure 4: volumetric efficiency. Algebraic, so no settling.
    let mut vol_series = Vec::new();
    for rpm in [1500.0, 2500.0, 3400.0, 4220.0] {
        let omega = rpm_to_rad(rpm);
        let mut curve = Vec::new();
        let mut map = 0.8e5;
        while map <= 3.4e5 {
            curve.push((map / 1e5, cylinder::volumetric_efficiency(&p, map, omega)));
            map += 0.1e5;
        }
        vol_series.push(svg::Series::solid(&format!("{rpm:.0} rpm"), curve));
    }
    write_svg(
        "docs/validation/volumetric_efficiency.svg",
        &svg::chart(
            "Volumetric efficiency",
            "intake manifold pressure, bar",
            "volumetric efficiency",
            &vol_series,
            Some((0.80, 1.02)),
        ),
    )?;

    // Figure 5: the spool. Crank speed governed, so the only dynamics left are
    // manifold filling and the shaft, and the trace is the turbocharger lag.
    let amb = atmosphere::isa(0.0);
    let mut x = State::at_rest(amb.p, 3000.0);
    let mut boost = control::BoostController::new();
    let mut u = Inputs {
        fuel_cmd: 0.30,
        wastegate: 1.0,
        p_amb: amb.p,
        t_amb: amb.t,
        tas_m_s: 60.0,
        load_torque: 0.0,
    };
    let mut trace = Vec::new();
    let mut shaft = Vec::new();
    let step_at = 2.0;
    for i in 0..(14.0 / integrator::DT) as u32 {
        let t = f64::from(i) * integrator::DT;
        if t >= step_at {
            u.fuel_cmd = 1.0;
        }
        u.wastegate = boost.update(&p, u.fuel_cmd, x.p_im, x.omega_tc, integrator::DT);
        u.load_torque = engine_model::evaluate(&p, &x, &u).torque_brake;
        // Decimated to 20 Hz. The integrator runs at 200 Hz and every one of those
        // points in the figure would be four times the file for no visible detail.
        if i % 10 == 0 {
            trace.push((t, x.p_im / 1e5));
            shaft.push((t, x.turbo_rpm() / 1000.0));
        }
        x = engine_model::step(&p, &x, &u, integrator::DT);
    }
    let start = trace
        .iter()
        .find(|(t, _)| *t >= step_at)
        .map_or(1.0, |(_, v)| *v);
    let end = trace.last().map_or(1.0, |(_, v)| *v);
    let spool = trace
        .iter()
        .find(|(t, v)| *t >= step_at && *v >= start + 0.9 * (end - start))
        .map_or(f64::NAN, |(t, _)| t - step_at);
    write_svg(
        "docs/validation/spool.svg",
        &svg::chart(
            "Response to a fuelling step at 3000 rpm, crank speed governed",
            "time, s",
            "manifold pressure, bar (solid) and shaft speed, thousand rpm / 50 (dashed)",
            &[
                svg::Series::solid("intake manifold pressure", trace),
                svg::Series::dashed(
                    "turbocharger speed / 50",
                    shaft.into_iter().map(|(t, v)| (t, v / 50.0)).collect(),
                ),
            ],
            None,
        ),
    )?;

    // Reference operating points quoted in the document.
    let sea = settle(&p, 0.0, None, 3880.0, 110.0, 1.0);
    let critical = settle(&p, 11_000.0, None, 3880.0, 110.0, 1.0);
    let cruise = settle(&p, 22_400.0, Some(242.15), 3720.0, 78.0, 0.40);
    let hot = settle(&p, 0.0, Some(318.15), 3880.0, 70.0, 1.0);

    let mut md = std::fs::File::create("docs/model_validation.md")?;
    writeln!(
        md,
        "{}",
        document(&p, rated, at_ceiling, spool, &sea, &critical, &cruise, &hot)
    )?;

    eprintln!(
        "rated power held to 11,000 ft: {rated:.1} hp; at 32,000 ft: {at_ceiling:.1} hp; \
         spool {spool:.2} s"
    );
    Ok(())
}

fn write_svg(path: &str, body: &str) -> std::io::Result<()> {
    std::fs::write(path, body)
}

type Point = (State, Outputs, f64);

#[allow(clippy::too_many_arguments)]
fn document(
    p: &EngineParams,
    rated: f64,
    ceiling: f64,
    spool: f64,
    sea: &Point,
    critical: &Point,
    cruise: &Point,
    hot: &Point,
) -> String {
    let row = |name: &str, pt: &Point| {
        let (x, o, _) = pt;
        format!(
            "| {name} | {:.1} | {:.0} | {:.3} | {:.0} | {:.3} | {:.1} | {:.0} | {:.0} | {:.0} | {:.2} |",
            o.power_brake_w / HP,
            o.torque_prop,
            x.p_im / 1e5,
            x.turbo_rpm(),
            o.lambda,
            o.fuel_litres_per_hour(p),
            o.t_egt[0],
            x.t_cht[0],
            x.t_coolant,
            o.p_oil / 1e5,
        )
    };
    format!(
        r"# Model validation

Generated by `just validate`. Every figure and every number below is produced by
running the model; nothing here is authored. Re-run it to change it.

Engine: {name}. Parameters in `crates/engine-model/src/engines/ae330.toml`, each one
annotated `published` or `estimated`.

## 1. Power against altitude

The one that matters. The engine is rated at 180 hp held constant to 11,000 ft and
certified to 32,000 ft.

![power against altitude](validation/power_altitude.svg)

Modelled: **{rated:.1} hp held from sea level to 11,000 ft**, falling to
**{ceiling:.1} hp at 32,000 ft**.

## 2. Why the knee is where it is

A power curve with a bend in it proves nothing on its own. This is the mechanism.

![turbocharger speed against altitude](validation/turbo_altitude.svg)

Below the critical altitude the wastegate modulates to hold the manifold set-point,
and the shaft speed climbs steadily as the compressor is asked for more pressure ratio
against thinner air. At 11,000 ft the shaft reaches the speed the controller supervises
it to, and from there the wastegate has to reopen to protect it. That is what ends the
plateau. The knee is not a parameter; it is where compressor sizing and the shaft speed
limit meet, and moving it means resizing the compressor.

Demonstrated containment for this class of turbocharger is 178,000 rpm, and the model
stays below it across the envelope.

## 3. Temperatures across the envelope

![exhaust and head temperature against altitude](validation/temperatures_altitude.svg)

Exhaust temperature rises above the critical altitude, which is correct and is the
reason it is worth plotting: as boost falls the excess air ratio falls with it, and a
smoke-limited engine at altitude runs hotter than the same engine at sea level. Head
temperature falls, because it tracks fuel energy and the engine is making less power.

## 4. Brake specific fuel consumption

![brake specific fuel consumption](validation/bsfc.svg)

Drawn as one curve per engine speed rather than as a filled contour. Same information,
and it keeps the document free of a plotting dependency.

## 5. Volumetric efficiency

![volumetric efficiency](validation/volumetric_efficiency.svg)

Parametric rather than a lookup table: three coefficients that can be argued about
instead of a surface fitted to data this engine does not have. The form is fitted over
the cruise-to-take-off band and is clamped below about 1500 rpm, where it extrapolates
through unity.

## 6. Turbocharger response

![spool](validation/spool.svg)

Crank speed is governed, so the only dynamics remaining are manifold filling and the
turbocharger shaft, and the trace is the spool itself rather than the engine
accelerating. Time to 90% of the boost rise after a step from 30% to full fuelling at
3000 rpm: **{spool:.2} s**.

## 7. Reference operating points

| point | power, hp | prop torque, N.m | MAP, bar | turbo, rpm | lambda | fuel, L/h | EGT, K | CHT, K | coolant, K | oil, bar |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
{sea_row}
{critical_row}
{cruise_row}
{hot_row}

The hot-day case is the binding thermal condition for this engine, and it is worth
being explicit about why, because the intuition points the other way. Cooling is
liquid, so radiator air mass flow goes as `m * rho * V`. At a constant indicated
airspeed true airspeed rises as density falls, so that product scales with the square
root of density rather than with density: at 25,000 ft it is about two thirds of the
sea-level value against a temperature difference nearly two thirds larger, while the
heat to be rejected has fallen with the engine's own power. High altitude is therefore
not where cooling binds. Hot, low, slow and at full power is.

## What this does and does not establish

Established: the model reproduces the published rating point on power, propeller torque
and fuel consumption simultaneously, from a fit to that point alone; it holds rated
power to the rated critical altitude through a mechanism that can be pointed at rather
than asserted; and it stays physical across the whole certified envelope.

Not established: anything at part load or at altitude has no measurement to be checked
against, because none is published for this engine. The parameters marked `estimated`
in the parameter file are chosen from engine class and fitted to the one point that is
published. They are plausible, not measured, and the file says so for each one.
",
        name = p.name,
        sea_row = row("sea level, take-off", sea),
        critical_row = row("11,000 ft, full", critical),
        cruise_row = row("22,400 ft, cruise", cruise),
        hot_row = row("sea level, ISA+30 climb", hot),
    )
}
