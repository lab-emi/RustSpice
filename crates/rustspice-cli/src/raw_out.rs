//! The `--raw` output of `rustspice run`, written while the deck runs: every `.op` / `.dc` plot as soon as its analysis
//! finishes and every `.tran` row as soon as the integrator produces it (a [`TranSink`]), so a transient result is
//! never held in memory. The bytes are those of a rawfile written after the last analysis (`.tran` with
//! `--internal-points` excepted: its row count is only known at the end, so its `No. Points:` value is written as
//! spaces and filled in afterwards, the way ngspice streams its rawfile; the value is padded with spaces).
//!
//! The data goes to an unnamed file in the rawfile's directory (Linux `O_TMPFILE`), which gets the name
//! `<raw>.raw.partial` and is renamed to `<raw>` once every analysis succeeded, as the former write-at-the-end did.
//! Failure semantics are those of that former write-at-the-end:
//! * a failed analysis leaves neither a rawfile nor a `.partial` file (dropping an unfinished [`RawOut`] closes the
//!   unnamed file);
//! * a run killed by a signal (`SIGTERM`, `SIGKILL`, a watchdog) leaves nothing either: the kernel frees the unnamed
//!   file;
//! * an I/O error (missing directory, full or read-only disk) does not stop the simulation: the run goes on without a
//!   rawfile, prints its results, and [`RawOut::finish`] returns the first error, which `main` reports with exit code
//!   74 (a harness-side failure, not a refusal of the deck).
//!
//! Where unnamed files are not available (another OS, a filesystem without `O_TMPFILE`) the data is written to
//! `<raw>.raw.partial` directly. The first two points hold there too (an unfinished [`RawOut`] removes the file), except
//! for a killed run: it leaves its `<raw>.raw.partial` behind.

use std::fs::{self, File, OpenOptions};
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
    /// The open file has no name (`O_TMPFILE`): it is linked as `tmp` by [`RawOut::finish`]. Otherwise it is `tmp`.
    unnamed: bool,
    /// Time spent encoding and writing, in microseconds.
    write_us: f64,
}

/// An unnamed regular file in `dir`, freed by the kernel when it is closed without having been linked.
#[cfg(target_os = "linux")]
fn create_unnamed(dir: &Path) -> io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt;
    // read access for the copy fallback of `link_unnamed`; mode as `File::create` (0o666 minus the umask)
    OpenOptions::new()
        .read(true)
        .write(true)
        .mode(0o666)
        .custom_flags(libc::O_TMPFILE)
        .open(dir)
}

#[cfg(not(target_os = "linux"))]
fn create_unnamed(_dir: &Path) -> io::Result<File> {
    Err(io::Error::from(io::ErrorKind::Unsupported))
}

/// Gives the unnamed file `f` the name `to` (which must not exist): `linkat` of its `/proc/self/fd` entry, or, when
/// that is not possible (no `/proc`), a copy of its contents into a new file `to`.
fn link_unnamed(f: &mut File, to: &Path) -> io::Result<()> {
    #[cfg(target_os = "linux")]
    {
        use std::ffi::CString;
        use std::os::fd::AsRawFd;
        use std::os::unix::ffi::OsStrExt;
        let src = CString::new(format!("/proc/self/fd/{}", f.as_raw_fd()))?;
        let dst = CString::new(to.as_os_str().as_bytes())?;
        // SAFETY: both paths are valid NUL-terminated strings that outlive the call
        let rc = unsafe {
            libc::linkat(
                libc::AT_FDCWD,
                src.as_ptr(),
                libc::AT_FDCWD,
                dst.as_ptr(),
                libc::AT_SYMLINK_FOLLOW,
            )
        };
        if rc == 0 {
            return Ok(());
        }
        let e = io::Error::last_os_error();
        if e.kind() == io::ErrorKind::AlreadyExists {
            return Err(e);
        }
    }
    copy_into_new(f, to)
}

/// The contents of `f` (from its start) written to a new file `to`.
fn copy_into_new(f: &mut File, to: &Path) -> io::Result<()> {
    let mut out = OpenOptions::new().write(true).create_new(true).open(to)?;
    let written = f
        .seek(SeekFrom::Start(0))
        .and_then(|_| io::copy(f, &mut out))
        .and_then(|_| out.flush());
    if written.is_err() {
        // the file created here, not one that existed
        let _ = fs::remove_file(to);
    }
    written
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
            unnamed: false,
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
            let dir = match self.path.parent() {
                Some(d) if !d.as_os_str().is_empty() => d,
                _ => Path::new("."),
            };
            // any failure of the unnamed file (no support, missing or read-only directory) is retried with the named
            // one, whose error is the one reported
            let opened = match create_unnamed(dir) {
                Ok(f) => {
                    self.unnamed = true;
                    // a stale `.partial` of an earlier run goes, as `File::create` truncated it
                    let _ = fs::remove_file(&self.tmp);
                    Ok(f)
                }
                Err(_) => File::create(&self.tmp),
            };
            self.state = match opened {
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
        self.remove_named();
    }

    /// Removes `<raw>.raw.partial` if this run created it (an unnamed file has no name to remove).
    fn remove_named(&self) {
        if !self.unnamed {
            let _ = fs::remove_file(&self.tmp);
        }
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
        let unnamed = self.unnamed;
        let tmp = self.tmp.clone();
        let done = match std::mem::replace(&mut self.state, State::Done) {
            State::Open(w) => w
                .into_inner()
                .map_err(io::IntoInnerError::into_error)
                .and_then(|mut f| {
                    if unnamed {
                        // a stale `.partial` of an earlier, killed run is replaced, as `File::create` did
                        match fs::remove_file(&tmp) {
                            Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e),
                            _ => {}
                        }
                        link_unnamed(&mut f, &tmp)?;
                    }
                    drop(f);
                    // from here on the `.partial` name exists and is ours
                    self.unnamed = false;
                    fs::rename(&tmp, &self.path)
                }),
            State::Failed(e) => Err(e),
            State::Pending | State::Done => unreachable!("writer() opened or failed"),
        };
        if let Err(e) = done {
            self.remove_named();
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
        self.remove_named();
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("raw_out_{name}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    /// The unnamed file gets its name by a link, or by a copy of its contents where it cannot be linked; neither
    /// replaces an existing file.
    #[test]
    fn an_unnamed_file_is_published_by_link_or_by_copy() {
        let dir = scratch("publish");
        let Ok(mut f) = create_unnamed(&dir) else {
            eprintln!("skipped: no unnamed files on this filesystem");
            return;
        };
        f.write_all(b"rows").unwrap();
        assert_eq!(
            fs::read_dir(&dir).unwrap().count(),
            0,
            "the file has no name"
        );
        let linked = dir.join("linked");
        link_unnamed(&mut f, &linked).unwrap();
        let copied = dir.join("copied");
        copy_into_new(&mut f, &copied).unwrap();
        for p in [&linked, &copied] {
            let mut s = String::new();
            File::open(p).unwrap().read_to_string(&mut s).unwrap();
            assert_eq!(s, "rows", "{}", p.display());
        }
        let e = link_unnamed(&mut f, &linked).unwrap_err();
        assert_eq!(e.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(
            copy_into_new(&mut f, &copied).unwrap_err().kind(),
            io::ErrorKind::AlreadyExists
        );
        assert_eq!(
            fs::read(&copied).unwrap(),
            b"rows",
            "an existing file is kept"
        );
        let _ = fs::remove_dir_all(&dir);
    }
}
