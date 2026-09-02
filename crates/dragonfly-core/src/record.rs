//! Recording a mission to Parquet, and the schema it is written in.
//!
//! One file serves the REPLAY screen, the prognostics work and the ML corpus, so
//! the three cannot describe the same flight differently.
//!
//! # The schema is generated, never typed
//!
//! Column names come from [`twin_core::channels::TABLE`],
//! [`twin_core::health::DESCRIPTORS`] and [`twin_core::indices::NAMES`] at
//! runtime. A hand-written list of ninety names agrees until someone inserts a
//! channel, and then every column after the insertion carries the wrong series
//! under the right name. Adding a channel changes the schema and nothing else.
//!
//! Deliberately absent: the raw residual and its sigma, recovered exactly from
//! `measured - predicted` and `residual / sigma`, so a third copy cannot
//! disagree with the two it is defined by; and the source ages, because
//! staleness is a property of a live feed.
use std::fs::File;
use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use arrow::array::{
    ArrayRef, BooleanArray, Float32Array, Float64Array, StringArray, UInt32Array, UInt64Array,
};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use parquet::arrow::ArrowWriter;
use parquet::basic::{Compression, ZstdLevel};
use parquet::file::properties::WriterProperties;

use twin_core::channels::{CHANNELS, TABLE};
use twin_core::health::{DESCRIPTORS, PARAMS};
use twin_core::indices::{INDICES, NAMES};

use crate::telemetry::Frame;

/// Rows buffered before a record batch is written.
///
/// Two hundred seconds at the 20 Hz publish rate. Large enough that the column
/// chunks compress well and small enough that a recording killed mid-mission
/// loses a bounded amount rather than everything since the last flush.
const BATCH_ROWS: usize = 4_000;

/// Parquet column name for a channel, parameter or index display name.
///
/// Display names are written for a readout, so they carry spaces and capitals
/// that no column name should: `OIL P` becomes `oil_p` and `EGT 3` becomes
/// `egt_3`. Mechanical rather than a lookup table, because a lookup table is the
/// drift this module exists to avoid.
fn slug(name: &str) -> String {
    name.to_ascii_lowercase().replace([' ', '-'], "_")
}

/// Every `f32` column, in the order [`sample`] fills them.
///
/// Paired with [`sample`] by a test rather than by care: the two are written
/// apart and a column added to one and not the other would silently shift every
/// series after it.
#[must_use]
pub fn float_columns() -> Vec<String> {
    let mut names = vec![
        "altitude_m".into(),
        "oat_k".into(),
        "p_amb_pa".into(),
        "ias_ms".into(),
        "isa_deviation_k".into(),
        "throttle_pct".into(),
        "load_pct".into(),
        "wastegate".into(),
        "fuel_remaining_pct".into(),
        "bus_v".into(),
        "vib_rms_g".into(),
        "vib_kurtosis".into(),
        "injection_1_ms".into(),
        "injection_2_ms".into(),
        "injection_3_ms".into(),
        "injection_4_ms".into(),
        "rms_pct".into(),
        "transient".into(),
        "innovation_pct".into(),
        "distance".into(),
        "distance_limit".into(),
        "cusum".into(),
        "cusum_limit".into(),
        "drift_since_s".into(),
        "redline_since_s".into(),
        "lead_time_s".into(),
    ];
    for c in &TABLE {
        names.push(slug(c.name));
    }
    for c in &TABLE {
        names.push(format!("{}_hat", slug(c.name)));
    }
    for c in &TABLE {
        names.push(format!("{}_resn", slug(c.name)));
    }
    for d in &DESCRIPTORS {
        names.push(format!("theta_{}", slug(d.name)));
    }
    for d in &DESCRIPTORS {
        names.push(format!("theta_{}_sigma", slug(d.name)));
    }
    for n in &NAMES {
        names.push(format!("health_{}", slug(n)));
    }
    for d in &DESCRIPTORS {
        names.push(format!("rul_{}_h", slug(d.name)));
    }
    for d in &DESCRIPTORS {
        names.push(format!("rul_{}_p10_h", slug(d.name)));
    }
    for d in &DESCRIPTORS {
        names.push(format!("rul_{}_p90_h", slug(d.name)));
    }
    for i in 0..twin_core::signature::HYPOTHESES {
        names.push(format!("posterior_{i}"));
    }
    names
}

/// Every `f32` value for one frame, in [`float_columns`] order.
///
/// `None` wherever the quantity does not exist for this frame: before the twin
/// has an estimate, or for a remaining life on a parameter that is not declining.
/// A missing remaining life is emphatically not zero, and writing it as zero
/// grounds a serviceable aircraft.
fn sample(frame: &Frame, rated_power_w: f64) -> Vec<Option<f32>> {
    let mut out: Vec<Option<f32>> = Vec::new();
    let mut put = |v: f64| out.push(if v.is_finite() { Some(v as f32) } else { None });

    put(f64::from(frame.altitude_m));
    put(f64::from(frame.oat_k));
    put(f64::from(frame.p_amb_pa));
    put(f64::from(frame.ias_ms));
    put(f64::from(frame.isa_deviation_k));
    put(f64::from(frame.throttle_pct));
    put(f64::from(frame.load_pct));
    put(f64::from(frame.wastegate));
    put(f64::from(frame.fuel_remaining_pct));
    put(f64::from(frame.bus_v));
    put(f64::from(frame.vib_rms_g));
    put(f64::from(frame.vib_kurtosis));
    // An input, not a compared channel, so the measurement vector does not carry
    // it. Without it a recording cannot re-drive the twin, which is most of what
    // a recording is for.
    for v in frame.injection_ms {
        put(f64::from(v));
    }

    let twin = frame.twin.as_ref();
    let nan = f64::NAN;
    put(twin.map_or(nan, |t| t.rms_pct));
    put(twin.map_or(nan, |t| t.transient));
    put(twin.map_or(nan, |t| t.innovation_pct));
    put(twin.map_or(nan, |t| t.detection.distance));
    put(twin.map_or(nan, |t| t.detection.distance_limit));
    put(twin.map_or(nan, |t| t.detection.cusum));
    put(twin.map_or(nan, |t| t.detection.cusum_limit));
    put(twin.and_then(|t| t.detection.drift_since).unwrap_or(nan));
    put(twin.and_then(|t| t.detection.redline_since).unwrap_or(nan));
    put(twin.and_then(|t| t.detection.lead_time_s).unwrap_or(nan));

    // The measurement vector rather than the frame's own fields, so the measured
    // column and the prediction beside it are indexed by the same table and a
    // channel inserted upstream cannot pair them off by one.
    let measured = frame.measurement(rated_power_w).vector();
    for v in measured {
        put(v);
    }
    for i in 0..CHANNELS {
        put(twin.map_or(nan, |t| t.predicted[i]));
    }
    for i in 0..CHANNELS {
        put(twin.map_or(nan, |t| t.normalised[i]));
    }
    for i in 0..PARAMS {
        put(twin.map_or(nan, |t| t.theta[i]));
    }
    for i in 0..PARAMS {
        put(twin.map_or(nan, |t| t.theta_sigma[i]));
    }
    for i in 0..INDICES {
        put(twin.map_or(nan, |t| t.health[i]));
    }
    let prognosis = frame.prognosis.as_ref();
    for i in 0..PARAMS {
        put(prognosis.and_then(|p| p.parameter[i].hours).unwrap_or(nan));
    }
    for i in 0..PARAMS {
        put(prognosis.and_then(|p| p.parameter[i].p10).unwrap_or(nan));
    }
    for i in 0..PARAMS {
        put(prognosis.and_then(|p| p.parameter[i].p90).unwrap_or(nan));
    }
    for i in 0..twin_core::signature::HYPOTHESES {
        put(twin.map_or(nan, |t| t.diagnosis.posterior[i]));
    }
    out
}

/// The Parquet schema: the fixed columns, then everything [`float_columns`] names.
#[must_use]
pub fn schema() -> Arc<Schema> {
    let mut fields = vec![
        Field::new("seq", DataType::UInt64, false),
        Field::new("t_s", DataType::Float64, false),
        Field::new("link_ok", DataType::Boolean, false),
        Field::new("twin_locked", DataType::Boolean, false),
        Field::new("calibrating", DataType::Boolean, false),
        Field::new("anomaly", DataType::Boolean, false),
        Field::new("drift", DataType::Boolean, false),
        Field::new("engine_state", DataType::Utf8, false),
        Field::new("flags", DataType::UInt32, false),
        Field::new("cusum_channel", DataType::Utf8, false),
        Field::new("redline_channel", DataType::Utf8, false),
        Field::new("diagnosis_best", DataType::UInt32, false),
    ];
    fields.extend(
        float_columns()
            .into_iter()
            .map(|n| Field::new(n, DataType::Float32, true)),
    );
    Arc::new(Schema::new(fields))
}

/// Columns accumulated between flushes.
#[derive(Default)]
struct Buffers {
    seq: Vec<u64>,
    t_s: Vec<f64>,
    link_ok: Vec<bool>,
    twin_locked: Vec<bool>,
    calibrating: Vec<bool>,
    anomaly: Vec<bool>,
    drift: Vec<bool>,
    engine_state: Vec<String>,
    flags: Vec<u32>,
    cusum_channel: Vec<String>,
    redline_channel: Vec<String>,
    diagnosis_best: Vec<u32>,
    floats: Vec<Vec<Option<f32>>>,
}

impl Buffers {
    fn len(&self) -> usize {
        self.seq.len()
    }
}

/// Writes frames to a Parquet file.
///
/// Both the live recorder and the offline generator go through this, which is
/// what makes a generated mission and a flown one the same file format produced
/// by the same code.
pub struct Recorder {
    writer: ArrowWriter<File>,
    schema: Arc<Schema>,
    rated_power_w: f64,
    buffers: Buffers,
    rows: u64,
}

impl Recorder {
    /// Create a recording at `path`, overwriting anything there.
    ///
    /// `rated_power_w` is the denominator the torque channel is reconstructed
    /// with, and it must be the same figure the twin was given or the torque
    /// column and the prediction beside it describe different engines.
    pub fn create(path: &Path, rated_power_w: f64) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).with_context(|| format!("{}", parent.display()))?;
        }
        let schema = schema();
        let file = File::create(path).with_context(|| format!("{}", path.display()))?;
        // Zstd rather than snappy: most of these columns are a constant or a
        // slow ramp, several are entirely null on a healthy engine, and the file
        // is written once and read many times.
        let props = WriterProperties::builder()
            .set_compression(Compression::ZSTD(ZstdLevel::default()))
            .build();
        let writer = ArrowWriter::try_new(file, Arc::clone(&schema), Some(props))
            .context("opening the parquet writer")?;
        Ok(Self {
            writer,
            schema,
            rated_power_w,
            buffers: Buffers::default(),
            rows: 0,
        })
    }

    /// Append one frame, flushing a batch once enough have accumulated.
    pub fn push(&mut self, frame: &Frame) -> Result<()> {
        let b = &mut self.buffers;
        let twin = frame.twin.as_ref();
        b.seq.push(frame.seq);
        b.t_s.push(frame.t_s);
        b.link_ok.push(frame.link_ok);
        b.twin_locked.push(twin.is_some_and(|t| t.locked));
        b.calibrating
            .push(twin.is_some_and(|t| t.detection.calibrating));
        b.anomaly.push(twin.is_some_and(|t| t.detection.anomaly));
        b.drift.push(twin.is_some_and(|t| t.detection.drift));
        b.engine_state.push(frame.engine_state.to_owned());
        b.flags.push(frame.flags);
        b.cusum_channel
            .push(twin.map_or("", |t| t.detection.cusum_channel).to_owned());
        b.redline_channel
            .push(twin.map_or("", |t| t.detection.redline_channel).to_owned());
        b.diagnosis_best
            .push(twin.map_or(0, |t| t.diagnosis.best as u32));
        b.floats.push(sample(frame, self.rated_power_w));
        self.rows += 1;

        if self.buffers.len() >= BATCH_ROWS {
            self.flush()?;
        }
        Ok(())
    }

    /// Write what is buffered as one record batch.
    fn flush(&mut self) -> Result<()> {
        if self.buffers.len() == 0 {
            return Ok(());
        }
        let b = std::mem::take(&mut self.buffers);
        let width = b.floats.first().map_or(0, Vec::len);
        let mut columns: Vec<ArrayRef> = vec![
            Arc::new(UInt64Array::from(b.seq)),
            Arc::new(Float64Array::from(b.t_s)),
            Arc::new(BooleanArray::from(b.link_ok)),
            Arc::new(BooleanArray::from(b.twin_locked)),
            Arc::new(BooleanArray::from(b.calibrating)),
            Arc::new(BooleanArray::from(b.anomaly)),
            Arc::new(BooleanArray::from(b.drift)),
            Arc::new(StringArray::from(b.engine_state)),
            Arc::new(UInt32Array::from(b.flags)),
            Arc::new(StringArray::from(b.cusum_channel)),
            Arc::new(StringArray::from(b.redline_channel)),
            Arc::new(UInt32Array::from(b.diagnosis_best)),
        ];
        // Transposed here rather than accumulated column-wise, because a row is
        // what arrives and holding ninety open builders to avoid one transpose
        // per flush costs more code than the transpose does.
        for c in 0..width {
            let column: Vec<Option<f32>> = b.floats.iter().map(|row| row[c]).collect();
            columns.push(Arc::new(Float32Array::from(column)));
        }
        let batch = RecordBatch::try_new(Arc::clone(&self.schema), columns)
            .context("building a record batch")?;
        self.writer.write(&batch).context("writing a batch")?;
        Ok(())
    }

    /// Flush and close the file. A recording not finished has no footer and
    /// cannot be opened, so this is not optional.
    pub fn finish(mut self) -> Result<u64> {
        self.flush()?;
        self.writer.finish().context("closing the parquet file")?;
        Ok(self.rows)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Columns written before the generated float columns begin: the identity,
    /// the flags a replay gates on, and the fields that are text or a bitmask.
    const FIXED_COLUMNS: usize = 12;

    /// The one invariant holding the schema together: names and values are
    /// written in two places and a column added to one and not the other would
    /// shift every series after it under a name that still looks right.
    #[test]
    fn every_named_column_is_filled() {
        let names = float_columns();
        let frame = crate::telemetry::Frame {
            twin: None,
            prognosis: None,
            ..bare_frame()
        };
        assert_eq!(names.len(), sample(&frame, 132_000.0).len());
        assert_eq!(schema().fields().len(), names.len() + FIXED_COLUMNS);
    }

    /// Display names carry spaces and capitals; column names must not.
    #[test]
    fn channel_names_become_column_names() {
        let names = float_columns();
        assert!(names.contains(&"oil_p".to_owned()));
        assert!(names.contains(&"egt_3_hat".to_owned()));
        assert!(names.contains(&"lambda_3_resn".to_owned()));
        assert!(names.iter().all(|n| !n.contains(' ')));
    }

    pub(crate) fn bare_frame() -> Frame {
        Frame {
            seq: 1,
            t_s: 0.0,
            link_ok: true,
            ages: crate::telemetry::SourceAges::default(),
            altitude_m: 6828.0,
            oat_k: 242.15,
            p_amb_pa: 42_070.0,
            ias_ms: 40.1,
            isa_deviation_k: 0.0,
            throttle_pct: 38.0,
            load_pct: 60.0,
            rpm: 3720.0,
            map_pa: 118_000.0,
            mat_k: 320.0,
            boost_pa: 75_930.0,
            maf_kgs: 0.084,
            fuel_flow_kgh: 12.3,
            fuel_flow_lph: 15.3,
            lambda: 1.68,
            lambda_k: [1.68; 4],
            cht_k: [412.0; 4],
            egt_k: [843.0; 4],
            injection_ms: [1.5; 4],
            oil_p_pa: 420_000.0,
            oil_t_k: 358.0,
            coolant_t_k: 361.0,
            tc_rpm: 118_400.0,
            bus_v: 27.8,
            vib_rms_g: 2.0,
            vib_kurtosis: 2.1,
            wastegate: 0.2,
            fuel_remaining_pct: 80.0,
            engine_state: "RUNNING",
            flags: 0,
            twin: None,
            prognosis: None,
        }
    }
}
