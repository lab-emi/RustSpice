//! The `--raw` output of `rustspice run`, written while the deck runs: every `.op` / `.dc` plot as soon as its analysis
//! finishes and every `.tran` row as soon as the integrator produces it (a [`TranSink`]), so a transient result is
//! never held in memory. The bytes are those of a rawfile written after the last analysis (`.tran` with
//! `--internal-points` excepted: its row count is only known at the end, so its `No. Points:` value is written as
//! spaces and filled in afterwards, the way ngspice streams its rawfile; the value is padded with spaces).
//!
//! The data goes to `<raw>.raw.partial`, renamed to `<raw>` once every analysis succeeded. Failure semantics are those
//! of the former write-at-the-end:
//! * a failed analysis leaves neither a rawfile nor a `.partial` file (dropping an unfinished [`RawOut`] removes it);
//! * an I/O error (missing directory, full or read-only disk) does not stop the simulation: the run goes on without a
//!   rawfile, prints its results, and [`RawOut::finish`] returns the first error, which `main` reports with exit code
//!   74 (a harness-side failure, not a refusal of the deck).

use std::fs::{self, File};
use std::io::{self, BufWriter, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

use rustspice_core::SimResult;
use rustspice_solver::TranSink;

use crate::bench::rawfile::{self, Var};

/// Write buffer of the rawfile: large enough that the system calls cost little, small next to the run's own
/// footprint (the whole result used to go through a 1 MiB buffer at the end).
const WRITE_BUFFER: usize = 128 << 10;

enum State {
    /// Nothing written yet: the file is created by the first plot.
    Pending,
    Open(BufWriter<File>),
    /// The first I/O error: nothing more is written.
    Failed(io::Error),
    /// Renamed to its final name.
    Done,
}

/// The rawfile of one `rustspice run --raw <path>`.
pub struct RawOut {
    path: PathBuf,
    tmp: PathBuf,
    state: State,
    /// Time spent encoding and writing, in microseconds.
    write_us: f64,
}

/// Closes a writer WITHOUT flushing what it still buffers (the file is about to be removed).
fn discard(w: BufWriter<File>) {
    let _ = w.into_parts();
}

impl RawOut {
    pub fn new(path: &Path) -> Self {
        Self {
            path: path.to_path_buf(),
            tmp: path.with_extension("raw.partial"),
            state: State::Pending,
            write_us: 0.0,
        }
    }

    /// Microseconds spent encoding and writing so far.
    pub fn write_us(&self) -> f64 {
        self.write_us
    }

    /// The writer, creating the file on first use; `None` after an I/O error.
    fn writer(&mut self) -> Option<&mut BufWriter<File>> {
        if matches!(self.state, State::Pending) {
            self.state = match File::create(&self.tmp) {
                Ok(f) => State::Open(BufWriter::with_capacity(WRITE_BUFFER, f)),
                Err(e) => State::Failed(e),
            };
        }
        match &mut self.state {
            State::Open(w) => Some(w),
            _ => None,
        }
    }

    /// Keeps the first I/O error, closes and removes the partial file: the run goes on without a rawfile.
    fn fail(&mut self, e: io::Error) {
        if let State::Open(w) = std::mem::replace(&mut self.state, State::Failed(e)) {
            discard(w);
        }
        let _ = fs::remove_file(&self.tmp);
    }

    /// Writes one complete plot (`.op`, `.dc`): `rows[point][var]`, the optional `axis` becomes variable 0.
    pub fn write_plot(
        &mut self,
        title: &str,
        plotname: &str,
        vars: &[Var],
        axis: Option<&[f64]>,
        rows: &[Vec<f64>],
    ) {
        let t0 = Instant::now();
        if let Some(w) = self.writer() {
            if let Err(e) = rawfile::write_rows(w, title, plotname, vars, axis, rows) {
                self.fail(e);
            }
        }
        self.write_us += t0.elapsed().as_secs_f64() * 1e6;
    }

    /// A sink writing the rows of one `.tran` analysis as a `Transient Analysis` plot: `vars` lists `time` and the
    /// written unknowns, `cols` their indices in the MNA solution (all of them unless device-internal nodes exist).
    /// Call [`RawTranSink::finish`] when the analysis succeeded.
    pub fn tran_sink<'a>(
        &'a mut self,
        title: &'a str,
        vars: &'a [Var],
        cols: &'a [usize],
    ) -> RawTranSink<'a> {
        RawTranSink {
            out: self,
            title,
            vars,
            cols,
            all_cols: false,
            row: Vec::new(),
            declared: None,
            points_at: None,
            rows: 0,
        }
    }

    /// Flushes the file and gives it its final name (a run without any plot leaves an empty file, as before).
    /// Returns the microseconds spent writing, or the first I/O error of the run.
    pub fn finish(mut self) -> io::Result<f64> {
        let t0 = Instant::now();
        self.writer();
        let done = match std::mem::replace(&mut self.state, State::Done) {
            State::Open(mut w) => w.flush().and_then(|()| {
                drop(w);
                fs::rename(&self.tmp, &self.path)
            }),
            State::Failed(e) => Err(e),
            State::Pending | State::Done => unreachable!("writer() opened or failed"),
        };
        if let Err(e) = done {
            let _ = fs::remove_file(&self.tmp);
            return Err(e);
        }
        Ok(self.write_us + t0.elapsed().as_secs_f64() * 1e6)
    }
}

impl Drop for RawOut {
    /// An unfinished rawfile (a failed run) is removed.
    fn drop(&mut self) {
        if matches!(self.state, State::Done) {
            return;
        }
        if let State::Open(w) = std::mem::replace(&mut self.state, State::Done) {
            discard(w);
        }
        let _ = fs::remove_file(&self.tmp);
    }
}

/// Streams the rows of one `.tran` analysis into a [`RawOut`] (see [`RawOut::tran_sink`]).
pub struct RawTranSink<'a> {
    out: &'a mut RawOut,
    title: &'a str,
    vars: &'a [Var],
    cols: &'a [usize],
    /// Every MNA unknown is written (no device-internal nodes): the row is `x` itself.
    all_cols: bool,
    /// Encoded row: time, then the written unknowns (little-endian `f64`).
    row: Vec<u8>,
    /// Row count written in the header (`None`: deferred, filled in by `finish`).
    declared: Option<usize>,
    /// Offset of a deferred `No. Points:` value in the file.
    points_at: Option<u64>,
    rows: usize,
}

impl RawTranSink<'_> {
    /// Completes the plot of a successful analysis: fills in a deferred row count. A row count other than the one
    /// written in the header would corrupt the file and is an internal error.
    pub fn finish(self) -> Result<(), String> {
        if let Some(n) = self.declared {
            if n != self.rows {
                return Err(format!(
                    "internal error: the transient analysis produced {} rows, {n} were announced",
                    self.rows
                ));
            }
        }
        let t0 = Instant::now();
        if let (Some(at), Some(w)) = (self.points_at, self.out.writer()) {
            let value = rawfile::deferred_points_value(self.rows);
            let patched = (|| -> io::Result<()> {
                w.seek(SeekFrom::Start(at))?;
                w.write_all(&value)?;
                w.seek(SeekFrom::End(0))?;
                Ok(())
            })();
            if let Err(e) = patched {
                self.out.fail(e);
            }
        }
        self.out.write_us += t0.elapsed().as_secs_f64() * 1e6;
        Ok(())
    }
}

impl TranSink for RawTranSink<'_> {
    fn begin(&mut self, rows: Option<usize>, dim: usize) -> SimResult<()> {
        let t0 = Instant::now();
        self.all_cols = dim == self.cols.len();
        self.declared = rows;
        self.row = Vec::with_capacity(self.vars.len() * 8);
        let (title, vars) = (self.title, self.vars);
        let mut points_at = None;
        if let Some(w) = self.out.writer() {
            let written = match rows {
                Some(n) => rawfile::write_header(w, title, "Transient Analysis", vars, n),
                None => w.stream_position().and_then(|start| {
                    let off = rawfile::write_header_deferred(w, title, "Transient Analysis", vars)?;
                    points_at = Some(start + off);
                    Ok(())
                }),
            };
            if let Err(e) = written {
                self.out.fail(e);
            }
        }
        self.points_at = points_at;
        self.out.write_us += t0.elapsed().as_secs_f64() * 1e6;
        Ok(())
    }

    fn row(&mut self, t: f64, x: &[f64]) -> SimResult<()> {
        self.rows += 1;
        if !matches!(self.out.state, State::Open(_)) {
            // an I/O error ended the rawfile; the simulation goes on
            return Ok(());
        }
        let t0 = Instant::now();
        self.row.clear();
        self.row.extend_from_slice(&t.to_le_bytes());
        if self.all_cols {
            for v in x {
                self.row.extend_from_slice(&v.to_le_bytes());
            }
        } else {
            for &c in self.cols {
                self.row.extend_from_slice(&x[c].to_le_bytes());
            }
        }
        if let State::Open(w) = &mut self.out.state {
            if let Err(e) = w.write_all(&self.row) {
                self.out.fail(e);
            }
        }
        self.out.write_us += t0.elapsed().as_secs_f64() * 1e6;
        Ok(())
    }
}
