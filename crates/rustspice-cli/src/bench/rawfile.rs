//! Reader and writer for the SPICE "rawfile" format as produced by `ngspice -b -r` (real data, binary or ASCII,
//! several plots per file). Pure std, never panics on malformed input.

use std::fmt;
use std::io::Write;
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
    let meta =
        std::fs::metadata(path).map_err(|e| RawError::Io(format!("{}: {e}", path.display())))?;
    if meta.len() > MAX_PLOT_BYTES {
        return Err(RawError::Budget(format!(
            "{} is {} bytes (> {} byte limit)",
            path.display(),
            meta.len(),
            MAX_PLOT_BYTES
        )));
    }
    let data = std::fs::read(path).map_err(|e| RawError::Io(format!("{}: {e}", path.display())))?;
    parse(&data)
}

fn read_line(data: &[u8], pos: &mut usize) -> Option<String> {
    if *pos >= data.len() {
        return None;
    }
    let rest = &data[*pos..];
    let (line, adv) = match rest.iter().position(|&b| b == b'\n') {
        Some(i) => (&rest[..i], i + 1),
        None => (rest, rest.len()),
    };
    *pos += adv;
    Some(
        String::from_utf8_lossy(line)
            .trim_end_matches('\r')
            .to_string(),
    )
}

/// Parse every plot in `data`.
pub fn parse(data: &[u8]) -> Result<Vec<Plot>, RawError> {
    let loaded = parse_all(data, false)?;
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
    parse_all(data, true)
}

/// [`read_file`] with [`parse_lenient`].
pub fn read_file_lenient(path: &Path) -> Result<Loaded, RawError> {
    let meta =
        std::fs::metadata(path).map_err(|e| RawError::Io(format!("{}: {e}", path.display())))?;
    if meta.len() > MAX_PLOT_BYTES {
        return Err(RawError::Budget(format!(
            "{} is {} bytes (> {} byte limit)",
            path.display(),
            meta.len(),
            MAX_PLOT_BYTES
        )));
    }
    let data = std::fs::read(path).map_err(|e| RawError::Io(format!("{}: {e}", path.display())))?;
    parse_lenient(&data)
}

fn parse_all(data: &[u8], lenient: bool) -> Result<Loaded, RawError> {
    let mut out = Loaded::default();
    let mut pos = 0usize;
    let mut any = false;
    loop {
        // Skip blank lines between plots.
        while pos < data.len() && (data[pos] == b'\n' || data[pos] == b'\r' || data[pos] == b' ') {
            pos += 1;
        }
        if pos >= data.len() {
            break;
        }
        any = true;
        match parse_plot(data, &mut pos, lenient)? {
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

fn parse_plot(data: &[u8], pos: &mut usize, lenient: bool) -> Result<Parsed, RawError> {
    let mut title = String::new();
    let mut plotname = String::new();
    let mut flags = String::new();
    let mut nvars: Option<usize> = None;
    let mut npoints: Option<usize> = None;
    let mut vars: Vec<Var> = Vec::new();
    let mut binary = None;
    let mut header_lines = 0usize;
    while let Some(line) = read_line(data, pos) {
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
                let Some(l) = read_line(data, pos) else {
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
            let avail = data.len().saturating_sub(*pos) as u128;
            if need > avail {
                return Err(RawError::Truncated(format!(
                    "complex plot `{plotname}` needs {need} data bytes, {avail} available"
                )));
            }
            *pos += need as usize;
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
    let mut cols: Vec<Vec<f64>> = (0..nv).map(|_| Vec::new()).collect();
    if binary {
        let need = bytes as usize;
        if data.len() - (*pos).min(data.len()) < need {
            return Err(RawError::Truncated(format!(
                "plot `{plotname}` needs {need} data bytes, {} available",
                data.len().saturating_sub(*pos)
            )));
        }
        let block = &data[*pos..*pos + need];
        *pos += need;
        for c in cols.iter_mut() {
            c.reserve_exact(np);
        }
        for (k, i) in (0..need).step_by(8).enumerate() {
            let mut b = [0u8; 8];
            b.copy_from_slice(&block[i..i + 8]);
            cols[k % nv].push(f64::from_le_bytes(b));
        }
    } else {
        for p in 0..np {
            // point index, then nv values, whitespace separated (ngspice puts the index and first value on one line)
            let mut toks = Vec::with_capacity(nv + 1);
            while toks.len() < nv + 1 {
                let l = read_line(data, pos)
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
                cols[i].push(v);
            }
        }
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

fn write_header<W: Write>(
    w: &mut W,
    title: &str,
    plotname: &str,
    vars: &[Var],
    npoints: usize,
) -> std::io::Result<()> {
    write!(
        w,
        "Title: {}\nDate: n/a\nCommand: {HEADER_COMMAND}\nPlotname: {}\nFlags: real\nNo. Variables: {}\nNo. Points: {}\nVariables:\n",
        title.replace('\n', " "),
        plotname,
        vars.len(),
        npoints
    )?;
    for (i, v) in vars.iter().enumerate() {
        writeln!(w, "\t{}\t{}\t{}", i, v.name, v.kind)?;
    }
    w.write_all(b"Binary:\n")
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
