//! Reader and writer for the SPICE "rawfile" format as produced by `ngspice -b -r` (real data, binary or ASCII,
//! several plots per file). Pure std, never panics on malformed input.

use std::fmt;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::Path;

/// Largest plot (or file) the reader will load: `skipped_budget` territory above this (Rule 9).
pub const MAX_PLOT_BYTES: u64 = 1 << 30;

#[derive(Clone, Debug, PartialEq)]
pub struct Var {
    /// Lower-case vector name, e.g. `time`, `v(out)`, `i(v1)`.
    pub name: String,
    /// `time`, `voltage`, `current`, ...
    pub kind: String,
}

/// One plot: variables with equally long columns (`cols[var][point]`).
#[derive(Clone, Debug, PartialEq)]
pub struct Plot {
    pub title: String,
    pub plotname: String,
    pub vars: Vec<Var>,
    pub npoints: usize,
    pub cols: Vec<Vec<f64>>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RawError {
    Io(String),
    /// A plot or the file exceeds [`MAX_PLOT_BYTES`].
    Budget(String),
    Malformed(String),
    Truncated(String),
    Unsupported(String),
}

impl fmt::Display for RawError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RawError::Io(s) => write!(f, "rawfile i/o error: {s}"),
            RawError::Budget(s) => write!(f, "rawfile over budget: {s}"),
            RawError::Malformed(s) => write!(f, "malformed rawfile: {s}"),
            RawError::Truncated(s) => write!(f, "truncated rawfile: {s}"),
            RawError::Unsupported(s) => write!(f, "unsupported rawfile: {s}"),
        }
    }
}

impl Plot {
    pub fn col(&self, name: &str) -> Option<&[f64]> {
        let name = name.to_ascii_lowercase();
        self.vars
            .iter()
            .position(|v| v.name == name)
            .map(|i| self.cols[i].as_slice())
    }

    /// Analysis kind from the plot name (`Operating Point`, `DC transfer characteristic`, `Transient Analysis`).
    pub fn kind(&self) -> Option<PlotKind> {
        let p = self.plotname.to_ascii_lowercase();
        if p.starts_with("operating") {
            Some(PlotKind::Op)
        } else if p.contains("dc transfer") {
            Some(PlotKind::Dc)
        } else if p.contains("transient") {
            Some(PlotKind::Tran)
        } else {
            None
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlotKind {
    Op,
    Dc,
    Tran,
}

impl PlotKind {
    pub fn plotname(self) -> &'static str {
        match self {
            PlotKind::Op => "Operating Point",
            PlotKind::Dc => "DC transfer characteristic",
            PlotKind::Tran => "Transient Analysis",
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            PlotKind::Op => "op",
            PlotKind::Dc => "dc",
            PlotKind::Tran => "tran",
        }
    }
}

/// Last plot of the given kind (an analysis may be preceded by other plots of the same kind).
pub fn last_plot(plots: &[Plot], kind: PlotKind) -> Option<&Plot> {
    plots.iter().rev().find(|p| p.kind() == Some(kind))
}

/// `v1#branch` -> `i(v1)`; everything lower-cased.
fn normalize_name(raw: &str) -> String {
    let n = raw.trim().to_ascii_lowercase();
    if let Some(stem) = n.strip_suffix("#branch") {
        return format!("i({stem})");
    }
    n
}

pub fn read_file(path: &Path) -> Result<Vec<Plot>, RawError> {
    Ok(read_path(path, false, None)?.plots)
}

/// Reads and parses the file at `path` through a buffer: only the parsed columns are ever held, never the file.
fn read_path(path: &Path, lenient: bool, keep: Option<&[String]>) -> Result<Loaded, RawError> {
    let io = |e: io::Error| RawError::Io(format!("{}: {e}", path.display()));
    let f = std::fs::File::open(path).map_err(io)?;
    let len = f.metadata().map_err(io)?.len();
    if len > MAX_PLOT_BYTES {
        return Err(RawError::Budget(format!(
            "{} is {} bytes (> {} byte limit)",
            path.display(),
            len,
            MAX_PLOT_BYTES
        )));
    }
    let mut src = Src {
        r: BufReader::with_capacity(64 << 10, f),
        pos: 0,
        len,
        name: path.display().to_string(),
    };
    parse_src(&mut src, lenient, keep)
}

/// The bytes of a rawfile being parsed: in memory (`&[u8]`) or a buffered file, with its length for the truncation
/// checks.
struct Src<R: BufRead> {
    r: R,
    /// Bytes consumed so far.
    pos: u64,
    len: u64,
    /// Names the file in I/O errors.
    name: String,
}

fn src_error(name: &str, e: io::Error) -> RawError {
    if e.kind() == io::ErrorKind::UnexpectedEof {
        RawError::Truncated(format!("{name}: {e}"))
    } else {
        RawError::Io(format!("{name}: {e}"))
    }
}

impl<'a> Src<&'a [u8]> {
    fn of(data: &'a [u8]) -> Self {
        Src {
            r: data,
            pos: 0,
            len: data.len() as u64,
            name: "rawfile".to_string(),
        }
    }
}

impl<R: BufRead> Src<R> {
    /// Bytes left after the current position.
    fn avail(&self) -> u64 {
        self.len.saturating_sub(self.pos)
    }

    /// The next line without its `\n` (and a trailing `\r`); `None` at the end of the data.
    fn read_line(&mut self) -> Result<Option<String>, RawError> {
        let mut line = Vec::new();
        let name = &self.name;
        let n = self
            .r
            .read_until(b'\n', &mut line)
            .map_err(|e| src_error(name, e))?;
        if n == 0 {
            return Ok(None);
        }
        self.pos += n as u64;
        if line.last() == Some(&b'\n') {
            line.pop();
        }
        Ok(Some(
            String::from_utf8_lossy(&line)
                .trim_end_matches('\r')
                .to_string(),
        ))
    }

    /// Skips newlines and spaces (between plots); `false` at the end of the data.
    fn skip_blank(&mut self) -> Result<bool, RawError> {
        loop {
            let name = &self.name;
            let buf = self.r.fill_buf().map_err(|e| src_error(name, e))?;
            if buf.is_empty() {
                return Ok(false);
            }
            let n = buf
                .iter()
                .take_while(|b| matches!(b, b'\n' | b'\r' | b' '))
                .count();
            let more = n < buf.len();
            self.r.consume(n);
            self.pos += n as u64;
            if more {
                return Ok(true);
            }
        }
    }

    fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), RawError> {
        let name = &self.name;
        self.r.read_exact(buf).map_err(|e| src_error(name, e))?;
        self.pos += buf.len() as u64;
        Ok(())
    }

    /// Skips `n` bytes (checked against [`Src::avail`] by the caller).
    fn skip(&mut self, n: u64) -> Result<(), RawError> {
        let name = &self.name;
        let done = io::copy(&mut (&mut self.r).take(n), &mut io::sink())
            .map_err(|e| src_error(name, e))?;
        self.pos += done;
        if done < n {
            return Err(RawError::Truncated(format!(
                "{name}: {n} bytes to skip, {done} available"
            )));
        }
        Ok(())
    }
}

/// Parse every plot in `data`.
pub fn parse(data: &[u8]) -> Result<Vec<Plot>, RawError> {
    let loaded = parse_all(data, false, None)?;
    Ok(loaded.plots)
}

/// Plots of a file read leniently: binary complex plots (`.ac`, `.pz`, `.disto`) are skipped, not an error.
#[derive(Clone, Debug, Default)]
pub struct Loaded {
    pub plots: Vec<Plot>,
    /// Plot names of the complex plots that were skipped.
    pub skipped: Vec<String>,
}

/// [`parse`] that skips binary complex plots instead of failing (the suite compares real plots only).
pub fn parse_lenient(data: &[u8]) -> Result<Loaded, RawError> {
    parse_all(data, true, None)
}

/// [`read_file`] with [`parse_lenient`].
pub fn read_file_lenient(path: &Path) -> Result<Loaded, RawError> {
    read_file_lenient_keep(path, None)
}

/// [`read_file_lenient`] keeping, of every plot, only the axis (the first variable) and the variables named in `keep`
/// (`None`: all of them). The file is read through a buffer and the other columns are skipped row by row, so a
/// reference with many vectors costs the memory of the few the comparison uses.
pub fn read_file_lenient_keep(path: &Path, keep: Option<&[String]>) -> Result<Loaded, RawError> {
    read_path(path, true, keep)
}

fn parse_all(data: &[u8], lenient: bool, keep: Option<&[String]>) -> Result<Loaded, RawError> {
    parse_src(&mut Src::of(data), lenient, keep)
}

fn parse_src<R: BufRead>(
    src: &mut Src<R>,
    lenient: bool,
    keep: Option<&[String]>,
) -> Result<Loaded, RawError> {
    let mut out = Loaded::default();
    let mut any = false;
    // blank lines between plots are skipped
    while src.skip_blank()? {
        any = true;
        match parse_plot(src, lenient, keep)? {
            Parsed::Plot(p) => out.plots.push(p),
            Parsed::Skipped(name) => out.skipped.push(name),
        }
    }
    if !any {
        return Err(RawError::Malformed("empty file".to_string()));
    }
    Ok(out)
}

enum Parsed {
    Plot(Plot),
    Skipped(String),
}

fn parse_plot<R: BufRead>(
    src: &mut Src<R>,
    lenient: bool,
    keep: Option<&[String]>,
) -> Result<Parsed, RawError> {
    let mut title = String::new();
    let mut plotname = String::new();
    let mut flags = String::new();
    let mut nvars: Option<usize> = None;
    let mut npoints: Option<usize> = None;
    let mut vars: Vec<Var> = Vec::new();
    let mut binary = None;
    let mut header_lines = 0usize;
    while let Some(line) = src.read_line()? {
        header_lines += 1;
        if header_lines > 1_000_000 {
            return Err(RawError::Malformed("header too long".to_string()));
        }
        if line.starts_with("Binary:") {
            binary = Some(true);
            break;
        }
        if line.starts_with("Values:") {
            binary = Some(false);
            break;
        }
        if line.starts_with("Variables:") {
            let nv = nvars
                .ok_or_else(|| RawError::Malformed("Variables before No. Variables".to_string()))?;
            if nv == 0 || nv > 10_000_000 {
                return Err(RawError::Malformed(format!("bad variable count {nv}")));
            }
            for i in 0..nv {
                let Some(l) = src.read_line()? else {
                    if npoints == Some(0) {
                        // a header-only file of an empty sweep (ngspice cuts the variable list off)
                        let cols = vec![Vec::new(); vars.len()];
                        return Ok(Parsed::Plot(Plot {
                            title,
                            plotname,
                            vars,
                            npoints: 0,
                            cols,
                        }));
                    }
                    return Err(RawError::Truncated("variable list".to_string()));
                };
                let parts: Vec<&str> = l.split_whitespace().collect();
                if parts.len() < 3 {
                    return Err(RawError::Malformed(format!("bad variable line {i}: `{l}`")));
                }
                vars.push(Var {
                    name: normalize_name(parts[1]),
                    kind: parts[2].to_ascii_lowercase(),
                });
            }
            continue;
        }
        if let Some((k, v)) = line.split_once(':') {
            let v = v.trim();
            match k.trim() {
                "Title" => title = v.to_string(),
                "Plotname" => plotname = v.to_string(),
                "Flags" => flags = v.to_ascii_lowercase(),
                "No. Variables" => {
                    nvars = Some(
                        v.parse()
                            .map_err(|_| RawError::Malformed(format!("No. Variables `{v}`")))?,
                    )
                }
                "No. Points" => {
                    npoints = Some(
                        v.parse()
                            .map_err(|_| RawError::Malformed(format!("No. Points `{v}`")))?,
                    )
                }
                _ => {}
            }
        } else if !line.trim().is_empty() {
            return Err(RawError::Malformed(format!(
                "unexpected header line `{line}`"
            )));
        }
    }
    let binary =
        binary.ok_or_else(|| RawError::Truncated("no Binary:/Values: marker".to_string()))?;
    let nv = nvars.ok_or_else(|| RawError::Malformed("missing No. Variables".to_string()))?;
    let np = npoints.ok_or_else(|| RawError::Malformed("missing No. Points".to_string()))?;
    if vars.len() != nv {
        return Err(RawError::Malformed(format!(
            "declared {nv} variables, found {}",
            vars.len()
        )));
    }
    if flags.contains("complex") {
        if lenient && binary {
            // 16 bytes per value (real and imaginary part)
            let need = (nv as u128) * (np as u128) * 16;
            let avail = src.avail() as u128;
            if need > avail {
                return Err(RawError::Truncated(format!(
                    "complex plot `{plotname}` needs {need} data bytes, {avail} available"
                )));
            }
            src.skip(need as u64)?;
            return Ok(Parsed::Skipped(plotname));
        }
        return Err(RawError::Unsupported("complex data".to_string()));
    }
    let bytes = (nv as u128) * (np as u128) * 8;
    if bytes > MAX_PLOT_BYTES as u128 {
        return Err(RawError::Budget(format!(
            "plot `{plotname}` is {nv} vars x {np} points = {bytes} bytes (> {MAX_PLOT_BYTES})"
        )));
    }
    // column of every variable in `cols`: all of them, or only the axis and the `keep` vectors
    let mut slot: Vec<Option<usize>> = Vec::with_capacity(nv);
    let mut kept = 0usize;
    for (i, v) in vars.iter().enumerate() {
        let read = i == 0 || keep.is_none_or(|names| names.contains(&v.name));
        slot.push(read.then_some(kept));
        kept += usize::from(read);
    }
    let mut cols: Vec<Vec<f64>> = (0..kept).map(|_| Vec::new()).collect();
    if binary {
        let need = bytes as u64;
        if src.avail() < need {
            return Err(RawError::Truncated(format!(
                "plot `{plotname}` needs {need} data bytes, {} available",
                src.avail()
            )));
        }
        for c in cols.iter_mut() {
            c.reserve_exact(np);
        }
        // row by row: (byte offset in the row, column) of the kept variables
        let picks: Vec<(usize, usize)> = slot
            .iter()
            .enumerate()
            .filter_map(|(i, s)| s.map(|c| (8 * i, c)))
            .collect();
        let mut row = vec![0u8; if np > 0 { 8 * nv } else { 0 }];
        for _ in 0..np {
            src.read_exact(&mut row)?;
            for &(at, c) in &picks {
                let mut b = [0u8; 8];
                b.copy_from_slice(&row[at..at + 8]);
                cols[c].push(f64::from_le_bytes(b));
            }
        }
    } else {
        for p in 0..np {
            // point index, then nv values, whitespace separated (ngspice puts the index and first value on one line)
            let mut toks = Vec::with_capacity(nv + 1);
            while toks.len() < nv + 1 {
                let l = src
                    .read_line()?
                    .ok_or_else(|| RawError::Truncated(format!("ASCII values at point {p}")))?;
                toks.extend(l.split_whitespace().map(|s| s.to_string()));
            }
            if toks.len() != nv + 1 {
                return Err(RawError::Malformed(format!(
                    "point {p}: {} tokens",
                    toks.len()
                )));
            }
            for (i, t) in toks[1..].iter().enumerate() {
                let v: f64 = t
                    .parse()
                    .map_err(|_| RawError::Malformed(format!("bad number `{t}` at point {p}")))?;
                if let Some(c) = slot[i] {
                    cols[c].push(v);
                }
            }
        }
    }
    if kept < nv {
        vars = vars
            .into_iter()
            .zip(&slot)
            .filter_map(|(v, c)| c.map(|_| v))
            .collect();
    }
    Ok(Parsed::Plot(Plot {
        title,
        plotname,
        vars,
        npoints: np,
        cols,
    }))
}

/// Text of the `Command:` header line. ngspice executes the value of that line as a command when it loads a rawfile
/// (`load file.raw` prints `Error: misplaced Command: line` and `<word>: no such command available in ngspice` for an
/// arbitrary word), but it accepts the producer line it writes itself, `Command: ngspice-47, Build ...`. Checked against
/// the black-box ngspice-47 executable: a value whose first word starts with `ngspice` loads without any output,
/// `rustspice`, an empty value and `n/a` do not. The line therefore carries an `ngspice-` prefixed, non-runnable text.
pub const HEADER_COMMAND: &str = "ngspice-compatible rawfile written by rustspice";

/// Width of the `No. Points:` value of a header written before the point count is known
/// ([`write_header_deferred`]): spaces, overwritten by [`deferred_points_value`] once the plot is complete (the way
/// ngspice streams its rawfile; readers trim the value).
pub const DEFERRED_POINTS_WIDTH: usize = 20;

/// Header text up to and including `No. Points: `.
fn header_head(title: &str, plotname: &str, vars: &[Var]) -> String {
    format!(
        "Title: {}\nDate: n/a\nCommand: {HEADER_COMMAND}\nPlotname: {}\nFlags: real\nNo. Variables: {}\nNo. Points: ",
        title.replace('\n', " "),
        plotname,
        vars.len(),
    )
}

/// Header text after the `No. Points:` value, through `Binary:`.
fn write_header_tail<W: Write>(w: &mut W, vars: &[Var]) -> std::io::Result<()> {
    w.write_all(b"\nVariables:\n")?;
    for (i, v) in vars.iter().enumerate() {
        writeln!(w, "\t{}\t{}\t{}", i, v.name, v.kind)?;
    }
    w.write_all(b"Binary:\n")
}

/// The header of a binary plot of `npoints` rows.
pub fn write_header<W: Write>(
    w: &mut W,
    title: &str,
    plotname: &str,
    vars: &[Var],
    npoints: usize,
) -> std::io::Result<()> {
    write!(w, "{}{}", header_head(title, plotname, vars), npoints)?;
    write_header_tail(w, vars)
}

/// [`write_header`] for a plot whose row count is not known yet: the `No. Points:` value is
/// [`DEFERRED_POINTS_WIDTH`] spaces. Returns the offset of that value from the first byte written, where
/// [`deferred_points_value`] goes once the rows are written.
pub fn write_header_deferred<W: Write>(
    w: &mut W,
    title: &str,
    plotname: &str,
    vars: &[Var],
) -> std::io::Result<u64> {
    let head = header_head(title, plotname, vars);
    w.write_all(head.as_bytes())?;
    w.write_all(&[b' '; DEFERRED_POINTS_WIDTH])?;
    write_header_tail(w, vars)?;
    Ok(head.len() as u64)
}

/// The `No. Points:` value of a deferred header: `npoints` left-aligned in [`DEFERRED_POINTS_WIDTH`] bytes.
pub fn deferred_points_value(npoints: usize) -> [u8; DEFERRED_POINTS_WIDTH] {
    let mut v = [b' '; DEFERRED_POINTS_WIDTH];
    let digits = npoints.to_string();
    v[..digits.len()].copy_from_slice(digits.as_bytes());
    v
}

/// Write plots in ngspice binary rawfile layout.
pub fn write<W: Write>(w: &mut W, plots: &[Plot]) -> std::io::Result<()> {
    for p in plots {
        write_header(w, &p.title, &p.plotname, &p.vars, p.npoints)?;
        let mut row = Vec::with_capacity(p.vars.len() * 8);
        for pt in 0..p.npoints {
            row.clear();
            for c in &p.cols {
                row.extend_from_slice(&c[pt].to_le_bytes());
            }
            w.write_all(&row)?;
        }
    }
    Ok(())
}

/// Write one plot straight from row-major data (`rows[point][var]`, the optional `axis` becomes variable 0);
/// avoids transposing large transient results. `vars` lists every variable including the axis.
pub fn write_rows<W: Write>(
    w: &mut W,
    title: &str,
    plotname: &str,
    vars: &[Var],
    axis: Option<&[f64]>,
    rows: &[Vec<f64>],
) -> std::io::Result<()> {
    write_header(w, title, plotname, vars, rows.len())?;
    let mut buf = Vec::with_capacity(vars.len() * 8);
    for (k, r) in rows.iter().enumerate() {
        buf.clear();
        if let Some(a) = axis {
            buf.extend_from_slice(&a[k].to_le_bytes());
        }
        for v in r {
            buf.extend_from_slice(&v.to_le_bytes());
        }
        w.write_all(&buf)?;
    }
    Ok(())
}

pub fn write_file(path: &Path, plots: &[Plot]) -> std::io::Result<()> {
    let f = std::fs::File::create(path)?;
    let mut w = std::io::BufWriter::with_capacity(1 << 20, f);
    write(&mut w, plots)?;
    w.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Vec<Plot> {
        vec![
            Plot {
                title: "t".into(),
                plotname: "Operating Point".into(),
                vars: vec![
                    Var {
                        name: "v(a)".into(),
                        kind: "voltage".into(),
                    },
                    Var {
                        name: "i(v1)".into(),
                        kind: "current".into(),
                    },
                ],
                npoints: 1,
                cols: vec![vec![1.5], vec![-2.5e-3]],
            },
            Plot {
                title: "t".into(),
                plotname: "Transient Analysis".into(),
                vars: vec![
                    Var {
                        name: "time".into(),
                        kind: "time".into(),
                    },
                    Var {
                        name: "v(a)".into(),
                        kind: "voltage".into(),
                    },
                ],
                npoints: 3,
                cols: vec![vec![0.0, 1e-9, 2e-9], vec![0.0, 0.5, f64::MIN_POSITIVE]],
            },
        ]
    }

    #[test]
    fn roundtrip_binary_multi_plot() {
        let plots = sample();
        let mut buf = Vec::new();
        write(&mut buf, &plots).unwrap();
        let back = parse(&buf).unwrap();
        assert_eq!(back, plots);
        assert_eq!(back[0].kind(), Some(PlotKind::Op));
        assert_eq!(
            last_plot(&back, PlotKind::Tran)
                .unwrap()
                .col("V(A)")
                .unwrap()[1],
            0.5
        );
    }

    #[test]
    fn write_rows_matches_column_writer() {
        let plots = sample();
        let tr = &plots[1];
        let rows: Vec<Vec<f64>> = (0..tr.npoints).map(|p| vec![tr.cols[1][p]]).collect();
        let mut a = Vec::new();
        write(&mut a, &plots[1..]).unwrap();
        let mut b = Vec::new();
        write_rows(
            &mut b,
            &tr.title,
            &tr.plotname,
            &tr.vars,
            Some(&tr.cols[0]),
            &rows,
        )
        .unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn a_deferred_point_count_parses_like_a_known_one() {
        let plots = sample();
        let tr = &plots[1];
        let rows: Vec<Vec<f64>> = (0..tr.npoints).map(|p| vec![tr.cols[1][p]]).collect();
        let mut known = Vec::new();
        write_rows(
            &mut known,
            &tr.title,
            &tr.plotname,
            &tr.vars,
            Some(&tr.cols[0]),
            &rows,
        )
        .unwrap();
        // the streamed form: blank count, rows, then the count written into the blank
        let mut streamed = Vec::new();
        let at = write_header_deferred(&mut streamed, &tr.title, &tr.plotname, &tr.vars).unwrap()
            as usize;
        for (k, r) in rows.iter().enumerate() {
            streamed.extend_from_slice(&tr.cols[0][k].to_le_bytes());
            streamed.extend_from_slice(&r[0].to_le_bytes());
        }
        streamed[at..at + DEFERRED_POINTS_WIDTH]
            .copy_from_slice(&deferred_points_value(tr.npoints));
        assert_eq!(parse(&streamed).unwrap(), parse(&known).unwrap());
        let text = String::from_utf8_lossy(&streamed);
        assert!(text.contains(&format!(
            "\nNo. Points: 3{}\n",
            " ".repeat(DEFERRED_POINTS_WIDTH - 1)
        )));
        assert_eq!(
            deferred_points_value(usize::MAX).len(),
            DEFERRED_POINTS_WIDTH
        );
    }

    #[test]
    fn keep_reads_only_the_axis_and_the_named_vectors() {
        let mut plots = sample();
        plots[1].vars.push(Var {
            name: "v(b)".into(),
            kind: "voltage".into(),
        });
        plots[1].cols.push(vec![7.0, 8.0, 9.0]);
        let mut buf = Vec::new();
        write(&mut buf, &plots).unwrap();
        let path = std::env::temp_dir().join(format!("rawfile_keep_{}.raw", std::process::id()));
        std::fs::write(&path, &buf).unwrap();
        let keep = vec!["v(b)".to_string()];
        let got = read_file_lenient_keep(&path, Some(&keep)).unwrap().plots;
        let _ = std::fs::remove_file(&path);
        // the axis (first variable) always stays; an absent name is simply not there
        assert_eq!(got[0].vars, plots[0].vars[..1]);
        assert_eq!(got[0].cols, plots[0].cols[..1]);
        let names: Vec<&str> = got[1].vars.iter().map(|v| v.name.as_str()).collect();
        assert_eq!(names, ["time", "v(b)"]);
        assert_eq!(
            got[1].cols,
            vec![plots[1].cols[0].clone(), plots[1].cols[2].clone()]
        );
        assert_eq!(got[1].npoints, 3);
        // ASCII plots too
        let txt = "Title: x\nPlotname: DC transfer characteristic\nFlags: real\nNo. Variables: 3\nNo. Points: 2\nVariables:\n\t0\tV(v-sweep)\tvoltage\n\t1\tV1#branch\tcurrent\n\t2\tv(b)\tvoltage\nValues:\n 0\t0.0\n\t-1.0e-3\n\t5\n 1\t1.0\n\t-2.0e-3\n\t6\n";
        let p = parse_all(txt.as_bytes(), true, Some(&keep)).unwrap().plots;
        assert_eq!(p[0].cols, vec![vec![0.0, 1.0], vec![5.0, 6.0]]);
    }

    #[test]
    fn ascii_fallback_and_branch_names() {
        let txt = "Title: x\nPlotname: DC transfer characteristic\nFlags: real\nNo. Variables: 2\nNo. Points: 2\nVariables:\n\t0\tV(v-sweep)\tvoltage\n\t1\tV1#branch\tcurrent\nValues:\n 0\t0.0\n\t-1.0e-3\n 1\t1.0\n\t-2.0e-3\n";
        let p = parse(txt.as_bytes()).unwrap();
        assert_eq!(p[0].vars[0].name, "v(v-sweep)");
        assert_eq!(p[0].vars[1].name, "i(v1)");
        assert_eq!(p[0].cols[1], vec![-1e-3, -2e-3]);
        assert_eq!(p[0].kind(), Some(PlotKind::Dc));
    }

    #[test]
    fn rejects_bad_headers_and_truncation() {
        assert!(matches!(parse(b""), Err(RawError::Malformed(_))));
        let mut buf = Vec::new();
        write(&mut buf, &sample()[..1]).unwrap();
        // truncated binary block
        assert!(matches!(
            parse(&buf[..buf.len() - 3]),
            Err(RawError::Truncated(_))
        ));
        // complex plots unsupported
        let cx = "Title: x\nPlotname: AC\nFlags: complex\nNo. Variables: 1\nNo. Points: 1\nVariables:\n\t0\tfrequency\tfrequency\nBinary:\n";
        assert!(matches!(
            parse(cx.as_bytes()),
            Err(RawError::Unsupported(_))
        ));
        // header counts that do not match
        let bad = "Title: x\nPlotname: Operating Point\nFlags: real\nNo. Variables: 2\nNo. Points: 1\nVariables:\n\t0\tv(a)\tvoltage\nBinary:\n";
        assert!(parse(bad.as_bytes()).is_err());
        // absurd size is refused without allocating
        let huge = "Title: x\nPlotname: Transient Analysis\nFlags: real\nNo. Variables: 100000\nNo. Points: 100000000000\nVariables:\n";
        let mut s = huge.to_string();
        for i in 0..100000 {
            s.push_str(&format!("\t{i}\tv(n{i})\tvoltage\n"));
        }
        s.push_str("Binary:\n");
        assert!(matches!(parse(s.as_bytes()), Err(RawError::Budget(_))));
    }

    #[test]
    fn malformed_fuzz_never_panics() {
        let mut base = Vec::new();
        write(&mut base, &sample()).unwrap();
        let mut state = 0x9e3779b97f4a7c15u64;
        let mut next = move || {
            state ^= state >> 12;
            state ^= state << 25;
            state ^= state >> 27;
            state.wrapping_mul(0x2545F4914F6CDD1D)
        };
        for _ in 0..3000 {
            let mut d = base.clone();
            match next() % 4 {
                0 => {
                    let n = (next() % 8 + 1) as usize;
                    for _ in 0..n {
                        let i = (next() as usize) % d.len();
                        d[i] = next() as u8;
                    }
                }
                1 => d.truncate((next() as usize) % d.len()),
                2 => {
                    let i = (next() as usize) % d.len();
                    let j = (next() as usize) % d.len();
                    let (a, b) = (i.min(j), i.max(j));
                    d.drain(a..b);
                }
                _ => {
                    let n = (next() % 64) as usize;
                    for _ in 0..n {
                        d.push(next() as u8);
                    }
                }
            }
            let _ = parse(&d);
        }
    }

    #[test]
    fn lenient_parse_skips_binary_complex_plots() {
        let hdr = b"Title: x\nPlotname: AC Analysis\nFlags: complex\nNo. Variables: 2\nNo. Points: 2\nVariables:\n\t0\tfrequency\tfrequency\n\t1\tv(a)\tvoltage\nBinary:\n";
        let mut buf = hdr.to_vec();
        buf.extend_from_slice(&[0u8; 2 * 2 * 16]);
        let ac_only = buf.clone();
        write(&mut buf, &sample()[..1]).unwrap();
        // the strict reader still refuses, the lenient one returns the real plot and names the skipped one
        assert!(matches!(parse(&buf), Err(RawError::Unsupported(_))));
        let l = parse_lenient(&buf).unwrap();
        assert_eq!(l.skipped, vec!["AC Analysis".to_string()]);
        assert_eq!(l.plots.len(), 1);
        assert_eq!(l.plots[0], sample()[0]);
        // an AC-only file has nothing to compare but is not an error
        let l = parse_lenient(&ac_only).unwrap();
        assert!(l.plots.is_empty() && l.skipped.len() == 1);
        // truncated complex data and an empty file are still errors
        assert!(matches!(
            parse_lenient(&ac_only[..ac_only.len() - 1]),
            Err(RawError::Truncated(_))
        ));
        assert!(parse_lenient(b"").is_err());
    }

    /// A file is read through a buffer, plot by plot and row by row: the same plots as parsing its bytes in memory,
    /// across buffer refills (rows wider than the buffer, a complex plot to skip, ASCII and binary plots), and the same
    /// errors for a truncated file.
    #[test]
    fn reading_a_file_gives_the_plots_of_its_bytes() {
        let wide: Vec<Var> = (0..9000)
            .map(|i| Var {
                name: if i == 0 {
                    "time".into()
                } else {
                    format!("v(n{i})")
                },
                kind: if i == 0 {
                    "time".into()
                } else {
                    "voltage".into()
                },
            })
            .collect();
        let rows: Vec<Vec<f64>> = (0..7)
            .map(|k| (1..9000).map(|i| (k * 9000 + i) as f64 * 0.5).collect())
            .collect();
        let axis: Vec<f64> = (0..7).map(|k| k as f64 * 1e-9).collect();
        let mut buf = b"Title: x\nPlotname: AC Analysis\nFlags: complex\nNo. Variables: 2\nNo. Points: 2\nVariables:\n\t0\tfrequency\tfrequency\n\t1\tv(a)\tvoltage\nBinary:\n".to_vec();
        buf.extend_from_slice(&[7u8; 2 * 2 * 16]);
        write_rows(
            &mut buf,
            "w",
            "Transient Analysis",
            &wide,
            Some(&axis),
            &rows,
        )
        .unwrap();
        buf.extend_from_slice(b"\n");
        write(&mut buf, &sample()).unwrap();
        buf.extend_from_slice(b"Title: x\nPlotname: DC transfer characteristic\nFlags: real\nNo. Variables: 2\nNo. Points: 2\nVariables:\n\t0\tV(v-sweep)\tvoltage\n\t1\tV1#branch\tcurrent\nValues:\n 0\t0.0\n\t-1.0e-3\n 1\t1.0\n\t-2.0e-3\n");
        let path = std::env::temp_dir().join(format!("rawfile_stream_{}.raw", std::process::id()));
        std::fs::write(&path, &buf).unwrap();
        let mem = parse_lenient(&buf).unwrap();
        let file = read_file_lenient(&path).unwrap();
        assert_eq!(file.plots, mem.plots);
        assert_eq!(file.skipped, mem.skipped);
        assert_eq!(file.plots.len(), 4);
        assert_eq!(file.plots[0].cols[8999][6], (6 * 9000 + 8999) as f64 * 0.5);
        let keep = vec!["v(n8999)".to_string(), "v(a)".to_string()];
        let kept = read_file_lenient_keep(&path, Some(&keep)).unwrap();
        assert_eq!(
            kept.plots,
            parse_all(&buf, true, Some(&keep)).unwrap().plots
        );
        assert_eq!(
            kept.plots[0].cols,
            vec![
                axis.clone(),
                rows.iter().map(|r| r[8998]).collect::<Vec<f64>>()
            ]
        );
        // the strict reader refuses the complex plot from a file as from memory
        assert_eq!(read_file(&path).unwrap_err(), parse(&buf).unwrap_err());
        // cut anywhere: the same plots or the same error as the bytes in memory (inside a header, the variable list
        // or the binary rows of the wide plot, and inside the last, ASCII plot)
        for cut in [100, 200, 70_000, 300_000, buf.len() - 40, buf.len() - 1] {
            std::fs::write(&path, &buf[..cut]).unwrap();
            let got = read_file_lenient(&path).map(|l| l.plots);
            assert_eq!(
                got,
                parse_lenient(&buf[..cut]).map(|l| l.plots),
                "cut at {cut}"
            );
            if cut <= 300_000 {
                assert!(got.is_err(), "cut at {cut}");
            }
        }
        let _ = std::fs::remove_file(&path);
        assert!(matches!(
            read_file(&path),
            Err(RawError::Io(m)) if m.contains("rawfile_stream_")
        ));
    }

    #[test]
    fn header_only_file_of_an_empty_sweep_is_a_zero_point_plot() {
        // what ngspice-47 writes for `.dc V1 0 5 -1`: the variable list is cut off after `No. Points: 0`
        let t = "Title: x\nPlotname: DC transfer characteristic\nFlags: real\nNo. Variables: 4\nNo. Points: 0       \nVariables:\n";
        let p = parse(t.as_bytes()).unwrap();
        assert_eq!((p[0].npoints, p[0].kind()), (0, Some(PlotKind::Dc)));
        // a truncated list with points declared stays an error
        let t = t.replace("No. Points: 0       ", "No. Points: 3");
        assert!(matches!(parse(t.as_bytes()), Err(RawError::Truncated(_))));
    }

    /// ER-process-14: ngspice executes a `Command:` header line on load, so the writer must not emit a runnable one.
    #[test]
    fn header_carries_no_runnable_command_line() {
        let mut buf = Vec::new();
        write(&mut buf, &sample()).unwrap();
        let text = String::from_utf8_lossy(&buf);
        for line in text.lines().filter(|l| l.starts_with("Command:")) {
            assert!(
                !line.starts_with("Command: rustspice"),
                "ngspice would run `rustspice` on load: {line}"
            );
            assert!(line.starts_with("Command: ngspice"), "{line}");
        }
        assert!(!text.contains("Command: rustspice"));
    }

    /// End to end against the black-box ngspice executable (`cargo test -- --ignored`): loading a file written by the
    /// writer through a `.control` block prints no error, exactly as for a file ngspice wrote itself. Skips (prints a
    /// notice, never passes silently) when ngspice is not installed.
    #[test]
    #[ignore]
    fn ngspice_loads_the_written_file_without_errors() {
        let dir = std::env::temp_dir().join(format!("rawfile_load_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let raw = dir.join("w.raw");
        write_file(&raw, &sample()).unwrap();
        let deck = dir.join("loader.sp");
        std::fs::write(
            &deck,
            "loader\nV9 a 0 1\nR9 a 0 1k\n.control\nload w.raw\nprint v(a)\n.endc\n.end\n",
        )
        .unwrap();
        let out = match std::process::Command::new("ngspice")
            .arg("-b")
            .arg("loader.sp")
            .current_dir(&dir)
            .output()
        {
            Ok(o) => o,
            Err(e) => {
                eprintln!("UNVERIFIED: ngspice not runnable ({e})");
                return;
            }
        };
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        let _ = std::fs::remove_dir_all(&dir);
        assert!(text.contains("Loading raw data file"), "{text}");
        for bad in ["misplaced", "no such command", "strange line"] {
            assert!(!text.contains(bad), "ngspice complained ({bad}):\n{text}");
        }
    }
}
