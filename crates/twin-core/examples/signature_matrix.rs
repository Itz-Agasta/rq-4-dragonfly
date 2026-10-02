//! Print the structured residual matrix, generated from the engine model.
//!
//! `just signatures` writes `docs/fault_signatures.md`. The document is generated
//! and never edited, for the same reason `docs/model_validation.md` is: a table of
//! fault signatures somebody typed is an assertion, and one the model produced is
//! a result. Every cell comes from running the perturbed engine to steady state.

use twin_core::channels::{CHANNELS, TABLE};
use twin_core::signature::{HYPOTHESES, NOMINAL, Signatures, catalogue};

fn main() -> std::io::Result<()> {
    let params = engine_model::engines::ae330();
    let signatures = Signatures::generate(&params);
    let rows = catalogue();

    let mut out = String::new();
    out.push_str("# Fault signature matrix\n\n");
    out.push_str(
        "This page lists the residual pattern each fault produces, which is what the ground station matches against to name a fault. `just signatures` generates it: each row comes from running the engine model with one parameter perturbed, settling it, and differencing the result against the healthy engine at the same operating point. Don't edit it by hand.\n\n",
    );
    out.push_str(&format!("Engine: {}\n\n", params.name));
    out.push_str(
        "The reference point is cruise: 3720 rpm at 22,400 ft on a standard day. Each row is a unit vector, the direction a fault pushes the residual, measured in each channel's own standard deviations with severity divided out. A row with one large cell is a single-channel fault, and a row spread across many cells isn't.\n\n",
    );
    out.push_str(
        "Channel names: MAP is manifold absolute pressure, MAT manifold air temperature, MAF mass air flow, EGT exhaust gas temperature, CHT cylinder head temperature, and LAMBDA the excess air ratio. A dot means the component is below 0.01.\n\n",
    );

    out.push_str("| hypothesis |");
    for c in &TABLE {
        out.push_str(&format!(" {} |", c.name));
    }
    out.push_str("\n| --- |");
    for _ in 0..CHANNELS {
        out.push_str(" ---: |");
    }
    out.push('\n');

    for (h, row) in rows.iter().enumerate() {
        if h == NOMINAL {
            continue;
        }
        out.push_str(&format!("| {} |", row.name));
        for v in signatures.row(h) {
            if v.abs() < 0.01 {
                out.push_str(" . |");
            } else {
                out.push_str(&format!(" {v:+.2} |"));
            }
        }
        out.push('\n');
    }

    out.push_str("\n## How well each pair of faults can be told apart\n\n");
    out.push_str(
        "Each cell is the cosine between two signatures. A value of 1 means the residual pattern alone can't separate the two faults, so something outside this matrix has to. A value of 0 means they're orthogonal, and any observation decides between them.\n\n",
    );
    out.push_str("| |");
    for (h, row) in rows.iter().enumerate() {
        if h != NOMINAL {
            out.push_str(&format!(" {} |", short(row.name)));
        }
    }
    out.push_str("\n| --- |");
    for _ in 1..HYPOTHESES {
        out.push_str(" ---: |");
    }
    out.push('\n');
    for (a, row) in rows.iter().enumerate() {
        if a == NOMINAL {
            continue;
        }
        out.push_str(&format!("| {} |", short(row.name)));
        for b in 0..HYPOTHESES {
            if b == NOMINAL {
                continue;
            }
            let c: f64 = signatures
                .row(a)
                .iter()
                .zip(signatures.row(b))
                .map(|(x, y)| x * y)
                .sum();
            out.push_str(&format!(" {c:+.2} |"));
        }
        out.push('\n');
    }

    std::fs::create_dir_all("docs")?;
    std::fs::write("docs/fault_signatures.md", &out)?;
    println!("{out}");
    Ok(())
}

/// An abbreviation short enough for a square matrix header.
fn short(name: &str) -> String {
    name.split_whitespace()
        .map(|w| w.chars().next().unwrap_or(' '))
        .collect()
}
