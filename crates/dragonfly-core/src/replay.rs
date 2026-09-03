//! Reading a recorded mission back into frames.
//!
//! Replay is a **data source swap, not a second renderer**: what comes out is the
//! same [`Frame`] the WebSocket carries, so the alert rules, the channel tables
//! and the strips work on a recording without knowing it is one.
//!
//! # What a replayed frame does not carry
//!
//! **No prognosis.** A [`prognostics::Rul`] carries a fitted rate and the span it
//! was fitted over and the recording holds neither, so synthesising one would put
//! a remaining life on screen that no fit produced.
//!
//! **No isolation detail.** `match_score`, `rejection` and the health-index
//! drivers are `NaN` and empty strings, so **a replayed frame must not be routed
//! to ANALYSIS**, which is what draws them.
//!
//! **No source ages.** Staleness is a property of a live feed, so they read zero;
//! `link_ok` is whatever was recorded, which is the half that is real.
use std::fs::File;
use std::path::Path;

use anyhow::{Context, Result, bail};
use arrow::array::{
    Array, BooleanArray, Float32Array, Float64Array, StringArray, UInt32Array, UInt64Array,
};
use arrow::record_batch::RecordBatch;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;

use twin_core::channels::{CHANNELS, TABLE};
use twin_core::health::PARAMS;
use twin_core::indices::INDICES;
use twin_core::signature::HYPOTHESES;
use twin_core::{Detection, Diagnosis, TwinOutput};

use crate::telemetry::{CYLINDERS, Frame, SourceAges};

/// One recorded mission, as the API describes it before anyone reads it.
#[derive(Clone, Debug, serde::Serialize)]
pub struct MissionInfo {
    /// File stem, which is what the API addresses a mission by.
    pub id: String,
    /// Frames in the recording.
    pub frames: usize,
    /// Mission time of the last frame, s.
    pub duration_s: f64,
    /// Bytes on disk.
    pub bytes: u64,
}

/// Every recording in `dir`, oldest first.
///
/// A directory that does not exist is an empty list rather than an error: a core
/// started with recording turned off has never made one, and a screen asking for
/// the list should get an empty list rather than a failure it has to explain.
pub fn list(dir: &Path) -> Result<Vec<MissionInfo>> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|e| e != "parquet") {
            continue;
        }
        let Some(id) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        // Read the footer rather than the file: a mission is megabytes and a
        // listing wants its length, so opening every recording to count rows
        // would make the list cost as much as the replay it precedes.
        let Ok(file) = File::open(&path) else {
            continue;
        };
        let stat = file.metadata().ok();
        let bytes = stat.as_ref().map_or(0, std::fs::Metadata::len);
        let modified = stat.and_then(|m| m.modified().ok());
        // **The mission being flown right now is here and cannot be opened.**
        // Parquet writes its schema and row groups in a footer at close, so a
        // recording in progress has no readable structure and nothing to replay.
        let Ok(builder) = ParquetRecordBatchReaderBuilder::try_new(file) else {
            continue;
        };
        let metadata = builder.metadata();
        let frames = metadata.file_metadata().num_rows().max(0) as usize;
        out.push((
            modified,
            MissionInfo {
                id: id.to_owned(),
                frames,
                duration_s: duration_from_statistics(metadata),
                bytes,
            },
        ));
    }
    // By write time, because the client takes the last entry as the newest and
    // the ids do not order chronologically: `mission-coking4h` sorts after every
    // `mission-<epoch>` whatever its age, so a recording made this morning lost
    // to one made last week. A file with no readable time sorts first, where it
    // cannot be mistaken for the latest.
    out.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.id.cmp(&b.1.id)));
    Ok(out.into_iter().map(|(_, info)| info).collect())
}

/// Mission length from the footer's column statistics, without reading a row.
///
/// The max of `t_s` is already in the footer the listing has open, so a directory
/// listing costs no row reads. `NaN` if the statistics are absent, which a writer
/// is permitted to do.
fn duration_from_statistics(metadata: &parquet::file::metadata::ParquetMetaData) -> f64 {
    let Some(column) = metadata
        .file_metadata()
        .schema_descr()
        .columns()
        .iter()
        .position(|c| c.name() == "t_s")
    else {
        return f64::NAN;
    };
    metadata
        .row_groups()
        .iter()
        .filter_map(|group| match group.column(column).statistics() {
            Some(parquet::file::statistics::Statistics::Double(stats)) => stats.max_opt().copied(),
            _ => None,
        })
        .fold(f64::NAN, f64::max)
}

/// Frames one request may return.
///
/// Measured on a four hour recording, 288,000 frames and 96 MB on disk: without
/// a cap one request serialises a 690 MB body and materialises every frame to do
/// it, so an unbounded read is a way to exhaust the daemon by asking it politely.
/// Twenty thousand is far more than a timeline 2,560 pixels wide can draw, and
/// caps a request at a measured 48 MB in 0.5 s.
pub const MAX_FRAMES: usize = 20_000;

/// Which part of a recording to read.
///
/// One type for the query string and for the reader, so the endpoint cannot
/// describe a window the reader interprets differently.
#[derive(Clone, Copy, Debug, Default, serde::Deserialize)]
pub struct Window {
    /// Keep one frame in `stride`. One by default, which is every frame.
    #[serde(default)]
    pub stride: Option<usize>,
    /// First frame to return, counted before striding.
    #[serde(default)]
    pub from: Option<usize>,
    /// How many frames to return after striding, capped at [`MAX_FRAMES`].
    #[serde(default)]
    pub count: Option<usize>,
}

impl Window {
    /// The window as three concrete numbers: first row, step, and how many.
    #[must_use]
    pub fn resolve(&self) -> (usize, usize, usize) {
        (
            self.from.unwrap_or(0),
            self.stride.unwrap_or(1).max(1),
            self.count.unwrap_or(MAX_FRAMES).clamp(1, MAX_FRAMES),
        )
    }
}

/// Read part of a recording back into frames.
///
/// **The window is pushed into the reader rather than applied to its result.**
/// The client holds a whole mission because scrubbing wants random access in
/// either direction; the daemon must not, because it would rebuild every frame
/// of the file on every request and the largest window any screen asks for is a
/// few thousand. `with_offset` and `with_limit` let parquet skip the row groups
/// outside the span, so a window late in a mission does not decompress the hours
/// before it.
pub fn read(path: &Path, window: &Window) -> Result<Vec<Frame>> {
    let (from, stride, count) = window.resolve();
    let file = File::open(path).with_context(|| format!("{}", path.display()))?;
    // The last frame of a strided window is `(count - 1) * stride` rows past the
    // first, so this is the exact span that yields `count` of them. A saturating
    // stride reads to the end of the file, which is the ceiling every strided
    // window already has: 182 ms on the 96 MB recording against the 235 ms the
    // overview costs.
    let span = stride
        .saturating_mul(count.saturating_sub(1))
        .saturating_add(1);
    let reader = ParquetRecordBatchReaderBuilder::try_new(file)
        .with_context(|| format!("{} is not readable parquet", path.display()))?
        .with_offset(from)
        .with_limit(span)
        .build()?;

    let mut frames = Vec::new();
    let mut row_index = 0usize;
    for batch in reader {
        let batch = batch.context("reading a record batch")?;
        let columns = Columns::new(&batch)?;
        for row in 0..batch.num_rows() {
            if row_index.is_multiple_of(stride) {
                frames.push(columns.frame(row));
            }
            row_index += 1;
        }
    }
    Ok(frames)
}

/// Column lookups for one record batch.
///
/// Resolved by name once per batch rather than per row. Positional access would
/// be faster and is exactly the failure this schema is generated to avoid: a
/// column inserted upstream would silently shift every series after it.
struct Columns<'a> {
    batch: &'a RecordBatch,
}

impl<'a> Columns<'a> {
    fn new(batch: &'a RecordBatch) -> Result<Self> {
        for required in ["seq", "t_s", "link_ok"] {
            if batch.column_by_name(required).is_none() {
                bail!("recording has no `{required}` column");
            }
        }
        Ok(Self { batch })
    }

    /// A float column's value, or `NaN` where the column or the value is absent.
    ///
    /// Missing reads as `NaN` rather than failing, so a recording from an older
    /// schema still replays with the channels it does have.
    fn f(&self, name: &str, row: usize) -> f64 {
        let Some(column) = self.batch.column_by_name(name) else {
            return f64::NAN;
        };
        let Some(values) = column.as_any().downcast_ref::<Float32Array>() else {
            return f64::NAN;
        };
        if values.is_null(row) {
            f64::NAN
        } else {
            f64::from(values.value(row))
        }
    }

    fn f32(&self, name: &str, row: usize) -> f32 {
        self.f(name, row) as f32
    }

    fn bool(&self, name: &str, row: usize) -> bool {
        self.batch
            .column_by_name(name)
            .and_then(|c| c.as_any().downcast_ref::<BooleanArray>())
            .is_some_and(|v| !v.is_null(row) && v.value(row))
    }

    fn u32(&self, name: &str, row: usize) -> u32 {
        self.batch
            .column_by_name(name)
            .and_then(|c| c.as_any().downcast_ref::<UInt32Array>())
            .filter(|v| !v.is_null(row))
            .map_or(0, |v| v.value(row))
    }

    fn text(&self, name: &str, row: usize) -> String {
        self.batch
            .column_by_name(name)
            .and_then(|c| c.as_any().downcast_ref::<StringArray>())
            .filter(|v| !v.is_null(row))
            .map_or_else(String::new, |v| v.value(row).to_owned())
    }

    /// A channel's value by its position in [`TABLE`], through the same slug the
    /// writer used, so the two cannot name a column differently.
    fn channel(&self, index: usize, suffix: &str, row: usize) -> f64 {
        let name = TABLE[index]
            .name
            .to_ascii_lowercase()
            .replace([' ', '-'], "_");
        self.f(&format!("{name}{suffix}"), row)
    }

    fn frame(&self, row: usize) -> Frame {
        let seq = self
            .batch
            .column_by_name("seq")
            .and_then(|c| c.as_any().downcast_ref::<UInt64Array>())
            .map_or(0, |v| v.value(row));
        let t_s = self
            .batch
            .column_by_name("t_s")
            .and_then(|c| c.as_any().downcast_ref::<Float64Array>())
            .map_or(0.0, |v| v.value(row));

        let p_amb_pa = self.f32("p_amb_pa", row);
        let map_pa = self.channel(ch::MAP, "", row) as f32;
        let lambda_k: [f32; CYLINDERS] =
            std::array::from_fn(|i| self.channel(ch::LAMBDA + i, "", row) as f32);
        let finite: Vec<f32> = lambda_k.iter().copied().filter(|v| v.is_finite()).collect();
        let lambda = if finite.is_empty() {
            f32::NAN
        } else {
            finite.iter().sum::<f32>() / finite.len() as f32
        };
        let fuel_flow_kgh = self.channel(ch::FUEL, "", row) as f32;

        Frame {
            seq,
            t_s,
            link_ok: self.bool("link_ok", row),
            ages: SourceAges::default(),
            altitude_m: self.f32("altitude_m", row),
            oat_k: self.f32("oat_k", row),
            p_amb_pa,
            ias_ms: self.f32("ias_ms", row),
            isa_deviation_k: self.f32("isa_deviation_k", row),
            throttle_pct: self.f32("throttle_pct", row),
            load_pct: self.f32("load_pct", row),
            rpm: self.channel(ch::RPM, "", row) as f32,
            map_pa,
            mat_k: self.channel(ch::MAT, "", row) as f32,
            // Derived on the way out rather than stored, because it is a
            // difference of two columns that are stored and a third copy could
            // only ever disagree with them.
            boost_pa: map_pa - p_amb_pa,
            maf_kgs: self.channel(ch::MAF, "", row) as f32,
            fuel_flow_kgh,
            fuel_flow_lph: fuel_flow_kgh / crate::telemetry::FUEL_DENSITY_KG_M3 * 1000.0,
            lambda,
            lambda_k,
            cht_k: std::array::from_fn(|i| self.channel(ch::CHT + i, "", row) as f32),
            egt_k: std::array::from_fn(|i| self.channel(ch::EGT + i, "", row) as f32),
            injection_ms: std::array::from_fn(|i| {
                self.f32(&format!("injection_{}_ms", i + 1), row)
            }),
            oil_p_pa: self.channel(ch::OIL_P, "", row) as f32,
            oil_t_k: self.channel(ch::OIL_T, "", row) as f32,
            coolant_t_k: self.channel(ch::COOLANT, "", row) as f32,
            tc_rpm: self.channel(ch::TURBO, "", row) as f32,
            bus_v: self.f32("bus_v", row),
            vib_rms_g: self.f32("vib_rms_g", row),
            vib_kurtosis: self.f32("vib_kurtosis", row),
            wastegate: self.f32("wastegate", row),
            fuel_remaining_pct: self.f32("fuel_remaining_pct", row),
            // Leaked as a static rather than carried: the frame wants a
            // `&'static str` and the recording holds one of four known words.
            engine_state: state_name(&self.text("engine_state", row)),
            flags: self.u32("flags", row),
            twin: self.twin(row),
            prognosis: None,
        }
    }

    /// The twin block, or `None` for a frame recorded before the filter had an
    /// estimate.
    ///
    /// Presence is decided by the first prediction rather than by a flag,
    /// because that is the column a consumer would draw and a flag could say a
    /// twin was present for a row that carries no prediction to show.
    fn twin(&self, row: usize) -> Option<TwinOutput> {
        if !self.channel(0, "_hat", row).is_finite() {
            return None;
        }
        let predicted: [f64; CHANNELS] = std::array::from_fn(|i| self.channel(i, "_hat", row));
        let normalised: [f64; CHANNELS] = std::array::from_fn(|i| self.channel(i, "_resn", row));
        let measured: [f64; CHANNELS] = std::array::from_fn(|i| self.channel(i, "", row));
        let residual: [f64; CHANNELS] = std::array::from_fn(|i| measured[i] - predicted[i]);
        // Recovered rather than stored: `normalised` is the residual over it, so
        // storing sigma as well would be a third number free to disagree with
        // the two it is defined by. Undefined where the residual is zero, which
        // is where nothing is drawn against it anyway.
        let sigma: [f64; CHANNELS] = std::array::from_fn(|i| residual[i] / normalised[i]);

        Some(TwinOutput {
            locked: self.bool("twin_locked", row),
            // No latch is stored, so a row reports only what that row knew. The
            // event log replays a mission from its start and re-derives the edge,
            // which is the only thing that reads this.
            ever_locked: self.bool("twin_locked", row),
            rms_pct: self.f("rms_pct", row),
            transient: self.f("transient", row),
            predicted,
            residual,
            sigma,
            normalised,
            theta: self.parameters("theta_", "", row),
            theta_sigma: self.parameters("theta_", "_sigma", row),
            innovation_pct: self.f("innovation_pct", row),
            health: std::array::from_fn(|i| {
                let name = twin_core::indices::NAMES[i]
                    .to_ascii_lowercase()
                    .replace([' ', '-'], "_");
                self.f(&format!("health_{name}"), row)
            }),
            health_driver: [""; INDICES],
            health_driver_value: [f64::NAN; INDICES],
            health_driver_limit: [f64::NAN; INDICES],
            detection: Detection {
                distance: self.f("distance", row),
                distance_limit: self.f("distance_limit", row),
                cusum: self.f("cusum", row),
                cusum_limit: self.f("cusum_limit", row),
                cusum_channel: channel_name(&self.text("cusum_channel", row)),
                calibrating: self.bool("calibrating", row),
                anomaly: self.bool("anomaly", row),
                drift: self.bool("drift", row),
                drift_since: finite(self.f("drift_since_s", row)),
                redline_since: finite(self.f("redline_since_s", row)),
                redline_channel: channel_name(&self.text("redline_channel", row)),
                lead_time_s: finite(self.f("lead_time_s", row)),
            },
            diagnosis: Diagnosis {
                posterior: std::array::from_fn(|h| self.f(&format!("posterior_{h}"), row)),
                match_score: [f64::NAN; HYPOTHESES],
                best: self.u32("diagnosis_best", row) as usize,
                // A recording stores the winner and not the fit, so a replayed
                // frame cannot say whether the library explained it. False rather
                // than a guess: `unexplained` suppresses a name, and inventing one
                // here would suppress names on recordings that were fine.
                unexplained: false,
                rejection: [""; HYPOTHESES],
            },
        })
    }

    fn parameters(&self, prefix: &str, suffix: &str, row: usize) -> [f64; PARAMS] {
        std::array::from_fn(|i| {
            let name = twin_core::health::DESCRIPTORS[i]
                .name
                .to_ascii_lowercase()
                .replace([' ', '-'], "_");
            self.f(&format!("{prefix}{name}{suffix}"), row)
        })
    }
}

/// Positions in [`TABLE`] this module needs by name.
mod ch {
    pub const RPM: usize = 0;
    pub const MAP: usize = 1;
    pub const MAT: usize = 2;
    pub const MAF: usize = 3;
    pub const TURBO: usize = 4;
    pub const FUEL: usize = 6;
    pub const OIL_P: usize = 7;
    pub const OIL_T: usize = 8;
    pub const COOLANT: usize = 9;
    pub const EGT: usize = 10;
    pub const CHT: usize = 14;
    pub const LAMBDA: usize = 18;
}

fn finite(value: f64) -> Option<f64> {
    value.is_finite().then_some(value)
}

/// Match a recorded engine state back to the static string the frame carries.
fn state_name(recorded: &str) -> &'static str {
    match recorded {
        "STARTING" => "STARTING",
        "RUNNING" => "RUNNING",
        "FAULT" => "FAULT",
        "STOPPED" => "STOPPED",
        // A recording written before this column existed reads empty here, and
        // defaulting that to STOPPED put "engine stopped" beside 3,721 rpm on the
        // four hour mission. Absent is not stopped; the screen shows a dash.
        _ => "",
    }
}

/// Match a recorded channel name back to the entry in [`TABLE`] it came from.
///
/// The alternative is leaking the string, which would leak once per row of every
/// recording ever opened.
fn channel_name(recorded: &str) -> &'static str {
    TABLE
        .iter()
        .find(|c| c.name == recorded)
        .map_or("", |c| c.name)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The named positions above are an index into a table that is allowed to
    /// change, and getting one wrong puts a coolant temperature in the oil
    /// pressure column with nothing failing.
    #[test]
    fn the_named_channel_positions_are_the_table_positions() {
        assert_eq!(TABLE[ch::RPM].name, "RPM");
        assert_eq!(TABLE[ch::MAP].name, "MAP");
        assert_eq!(TABLE[ch::MAT].name, "MAT");
        assert_eq!(TABLE[ch::MAF].name, "MAF");
        assert_eq!(TABLE[ch::TURBO].name, "TURBO");
        assert_eq!(TABLE[ch::FUEL].name, "FUEL");
        assert_eq!(TABLE[ch::OIL_P].name, "OIL P");
        assert_eq!(TABLE[ch::OIL_T].name, "OIL T");
        assert_eq!(TABLE[ch::COOLANT].name, "COOLANT");
        assert_eq!(TABLE[ch::EGT].name, "EGT 1");
        assert_eq!(TABLE[ch::CHT].name, "CHT 1");
        assert_eq!(TABLE[ch::LAMBDA].name, "LAMBDA 1");
    }

    /// The writer and the reader are the only two things that know the column
    /// layout, and nothing else would notice them disagreeing: a batch built in
    /// the wrong order still has the right types, so `RecordBatch::try_new`
    /// accepts it and every value lands one column over.
    #[test]
    fn a_frame_survives_being_written_and_read_back() {
        let path = std::env::temp_dir().join(format!(
            "dragonfly-roundtrip-{}.parquet",
            std::process::id()
        ));
        let mut frame = crate::record::tests::bare_frame();
        frame.flags = 0b1010_0001;
        let mut recorder = crate::record::Recorder::create(&path, 132_000.0).expect("a recorder");
        for seq in 1..=3u64 {
            frame.seq = seq;
            frame.t_s = seq as f64 * 0.05;
            frame.egt_k[2] = 700.0 + seq as f32;
            recorder.push(&frame).expect("a row");
        }
        recorder.finish().expect("a footer");

        let frames = read(&path, &Window::default()).expect("readable");
        std::fs::remove_file(&path).ok();

        assert_eq!(frames.len(), 3);
        assert_eq!(frames[2].seq, 3);
        assert!((frames[2].t_s - 0.15).abs() < 1e-9);
        assert!(
            (frames[2].egt_k[2] - 703.0).abs() < 0.01,
            "{}",
            frames[2].egt_k[2]
        );
        assert!(
            (frames[0].oil_p_pa - 420_000.0).abs() < 1.0,
            "{}",
            frames[0].oil_p_pa
        );
        assert!(
            (frames[0].coolant_t_k - 361.0).abs() < 0.01,
            "{}",
            frames[0].coolant_t_k
        );
        assert!((frames[0].injection_ms[2] - 1.5).abs() < 0.01);
        assert_eq!(frames[0].engine_state, "RUNNING");
        assert_eq!(frames[0].flags, 0b1010_0001, "the recorded status bitmask");
        assert!(
            frames[0].twin.is_none(),
            "a frame recorded with no estimate must read back with none"
        );
    }

    /// The window is what stops one request rebuilding a whole mission, so it
    /// has to be the reader that applies it. Slicing the result instead would
    /// pass this test and still materialise every frame in the file.
    #[test]
    fn the_reader_returns_only_the_window_asked_for() {
        let path =
            std::env::temp_dir().join(format!("dragonfly-window-{}.parquet", std::process::id()));
        let mut frame = crate::record::tests::bare_frame();
        let mut recorder = crate::record::Recorder::create(&path, 132_000.0).expect("a recorder");
        for seq in 0..100u64 {
            frame.seq = seq;
            recorder.push(&frame).expect("a row");
        }
        recorder.finish().expect("a footer");

        let window = Window {
            from: Some(10),
            stride: Some(5),
            count: Some(4),
        };
        let frames = read(&path, &window).expect("readable");
        std::fs::remove_file(&path).ok();

        let seqs: Vec<u64> = frames.iter().map(|f| f.seq).collect();
        assert_eq!(seqs, vec![10, 15, 20, 25]);
    }

    /// An absent `count` must not mean every frame in the file.
    #[test]
    fn an_unbounded_request_is_capped() {
        let (_, _, count) = Window::default().resolve();
        assert_eq!(count, MAX_FRAMES);
        let asked = Window {
            count: Some(usize::MAX),
            ..Window::default()
        };
        assert_eq!(asked.resolve().2, MAX_FRAMES);
    }

    #[test]
    fn a_missing_directory_lists_nothing() {
        let missions = list(Path::new("/nonexistent/missions")).expect("an empty list");
        assert!(missions.is_empty());
    }
}
