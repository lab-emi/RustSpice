//! Seeded random-circuit fuzzers and the malformed-deck mutator (owner: H2).
//!
//! Everything is deterministic (own xorshift64*, no clocks, no hash-map iteration order). The deck with index `k` of a
//! fuzz family depends only on `(seed, family, k)`, never on `count`, so a failing deck is reproduced by its tag
//! `seed:<n>` (the per-deck seed) or by regenerating with the same base seed and a count above its index.
//!
//! # Fuzz families (`fuzz_decks`; `count` decks of EACH family, group `robustness`, `expect: match`)
//!
//! | family | generator contract | oracle | validity filter |
//! |---|---|---|---|
//! | `fuzz_linear` | 3-12 nodes; MNA 1-norm condition number <= 1e8 ([`LINEAR_COND_MAX`], exact inverse); spanning tree of resistors with every node's parent drawn from ground or an earlier node (so the network is connected and grounded), 0..n extra resistors, R log-uniform over six decades `[10, 1e7]` ohm; 1-3 voltage sources `+-[0.5, 20]` V on distinct nodes to ground; 0-2 current sources `+-[1e-7, 1e-4]` A; 0-2 VCCS `+-[1e-6, 1e-3]` S | `dc_linear` | the oracle must solve the deck and all `|v| <= 1e4` V |
//! | `fuzz_diode` | 3-8 nodes, resistor tree `[100, 1e5]` ohm, 1-4 diodes, each in series with a resistor `[10, 1e5]` ohm (Newton well posed), `IS` log-uniform `[1e-16, 1e-12]`, `N` in {1, 1.2, 1.5}, random orientation; 1-2 sources `+-[0.5, 12]` V each behind a series resistor `[10, 1e4]`; with probability 0.3 a *stacked* source (`V2` between a new node and the node of `V1`, so one rail is the sum of two sources: tag `stacked_source`); a deck is tagged `clamp_sensitive` and carries `known_gap: clamp-rail` when its exact solution has a node outside the range spanned by its DC source values (RustSpice clamps Newton iterates to that range; evidence: audit a3, and 17 of 17 such decks failing against 0 of 108 others in a 125-deck calibration run) | `dc_nonlinear` | the oracle's Newton must converge |
//! | `fuzz_rc` | 3-8 nodes, resistor tree `[1e3, 1e4]` ohm rooted at n1 (one resistor to ground, plus 0-2 extra `[1e3, 1e5]` ones), a capacitor to ground per node plus 0-2 node-node capacitors, `C = Tref * 10^U(-0.5,0.5) / R_parent` so all time constants lie within about a decade of `Tref = 10^U(-8,-6)` s; one `PULSE(0 V2 td tr tf pw per)` source behind a series resistor `[1e3, 5e3]` whose delay (0-32 steps), edges (6-13 steps, never zero), width and period are integer multiples of the step `h = Tref/64` (edges on the step grid: the deck tests circuit correctness, not edge placement); `.tran h max(512 h, 1.5 per)` (ngspice `tmax = tstep`) | `tran_lti` | none needed (always well posed) |
//! | `fuzz_cmos` | 2-6 feed-forward CMOS gates (INV/NAND2/NOR2, MOS level 1 with the model cards of `examples/cmos_ring_oscillator.sp`), inputs from 3-4 DC sources at 0, 3.3 V or an intermediate level `[0.6, 2.7]` V (transition region), gate inputs from primary inputs or earlier gate outputs (a DAG: unique operating point), DC `.op`; at least one gate output is a clean logic one (a three-valued logic simulation of the DAG), so a probe is never vacuous | none | by construction |
//!
//! # Mutator (`mutants`)
//!
//! Token-level corruption of existing decks (the static corpus plus the families). Mutation kinds (the `<kind>` field of
//! the id `mut_<source id>_<kind>_<n>`): [`MUTATION_KINDS`]. Each mutant is a deck with `* bench.group: robustness`,
//! `* bench.expect: either`, no probes (the engine only classifies panic / abort / hang / OOM / exit status and compares
//! pass-or-fail class with ngspice). The three kinds the audit proved to be resource bombs carry
//! `bench.known_gap: resource-budget` (`bigtran`: `.tran 1p 1` aborts allocating 8 TB; `bigdc`: 1e9-point sweeps;
//! `recsub`: a self-recursive subcircuit overflows the stack), and so do the number kinds (`num1e999`, `numhuge`,
//! `longnum`) exactly when the replaced number is a `.dc` start/stop (or, once it overflows to infinity, step) or a
//! `.tran` tstop ([`sweep_bomb`], measured: the sweep or time axis is allocated without a budget). Every other
//! mutant is judged without a known gap. The structural bombs are only applied to sources RustSpice can parse
//! ([`bomb_eligible`]), and no other kind mutates a source that is a bomb itself, so a tagged mutant really reaches the
//! bomb and `gap_closed` cannot be a false alarm of the generator.
//! Kinds that need text outside ASCII (`nul`, `badutf`, `unicode`) inject the character itself (`U+0000`, `U+FFFD` which
//! is what a lossy decode of invalid bytes yields, `U+00E9`).

#![allow(clippy::needless_range_loop)]

use super::families::{num, Rng};
use super::oracle;
use super::types::GeneratedDeck;
use std::fmt::Write as _;

/// The four fuzz families in generation order.
pub const FUZZ_FAMILIES: [&str; 4] = ["fuzz_linear", "fuzz_diode", "fuzz_rc", "fuzz_cmos"];

/// All mutation kinds (cycled in this order by [`mutants`]).
pub const MUTATION_KINDS: [&str; 30] = [
    "dellin",   // delete one line (not the title)
    "duplin",   // duplicate one line
    "trunc",    // truncate the deck at a random character
    "numnan",   // a number becomes `nan`
    "numinf",   // a number becomes `inf`
    "num1e999", // a number becomes `1e999` (overflows f64 parsing)
    "numneg",   // a number is negated
    "numhuge",  // a number becomes `1e300`
    "numempty", // a number is removed
    "numzero",  // a number becomes `0`
    "longnum",  // a number gets 400 digits
    "dropend",  // `.end` removed
    "dupend",   // `.end` duplicated, with a trailing element after it
    "dropends", // every `.ends` removed (unterminated `.subckt`, or none)
    "dupends",  // `.ends` duplicated
    "unbal",    // `.subckt` without `.ends`
    "recsub",   // self-recursive subcircuit instantiated from the top level
    "deepsub",  // 200-deep chain of nested subcircuit instances (not recursive)
    "bigtran",  // `.tran 1p 1`: 1e12 points
    "bigdc",    // `.dc` with 1e9 points
    "longline", // one 100 000 character line
    "nul",      // NUL characters inside tokens
    "badutf",   // U+FFFD (lossy decode of invalid UTF-8) inside tokens
    "unicode",  // non-ASCII letters in element names
    "empty",    // title line only
    "notitle",  // first element line becomes the title
    "selfres",  // resistor with both terminals on the same node
    "vloop",    // two ideal voltage sources in parallel with different values
    "negr",     // negative resistance
    "swapcard", // `.op` / `.tran` / `.dc` cards shuffled (several analyses, duplicated)
];

fn sub_seed(seed: u64, fam: u64, k: u64) -> u64 {
    let mut r = Rng::new(
        seed ^ fam.wrapping_mul(0xA24B_AED4_963E_E407) ^ k.wrapping_mul(0x9FB2_1C65_1E98_DF25),
    );
    r.next_u64()
}

fn e5(x: f64) -> String {
    format!("{x:.5e}")
}

struct FuzzDeck {
    id: String,
    family: &'static str,
    probes: Vec<String>,
    tol_rel: Option<f64>,
    tol_abs: Option<f64>,
    tol_norm: Option<f64>,
    floor: Option<f64>,
    oracle: Option<&'static str>,
    size: u64,
    tags: Vec<String>,
    gap: Option<(&'static str, String)>,
    note: String,
    title: String,
    body: String,
}

fn render(d: FuzzDeck) -> GeneratedDeck {
    let mut s = String::with_capacity(d.body.len() + 700);
    let _ = writeln!(s, "{}", d.title);
    let _ = writeln!(s, "* bench.id: {}", d.id);
    let _ = writeln!(s, "* bench.group: robustness");
    let _ = writeln!(s, "* bench.title: {}", d.title);
    if !d.probes.is_empty() {
        let _ = writeln!(s, "* bench.probes: {}", d.probes.join(", "));
    }
    if let Some(v) = d.tol_rel {
        let _ = writeln!(s, "* bench.tol.rel: {v:e}");
    }
    if let Some(v) = d.tol_abs {
        let _ = writeln!(s, "* bench.tol.abs: {v:e}");
    }
    if let Some(v) = d.tol_norm {
        let _ = writeln!(s, "* bench.tol.norm: {v:e}");
    }
    if let Some(v) = d.floor {
        let _ = writeln!(s, "* bench.floor: {v:e}");
    }
    let _ = writeln!(s, "* bench.expect: match");
    if let Some((slug, note)) = &d.gap {
        let _ = writeln!(s, "* bench.known_gap: {slug}");
        let _ = writeln!(s, "* bench.gap.note: {note}");
    }
    if let Some(o) = d.oracle {
        let _ = writeln!(s, "* bench.oracle: {o}");
    }
    let _ = writeln!(s, "* bench.size: {}", d.size);
    let _ = writeln!(s, "* bench.est_rss_mib: 16");
    let _ = writeln!(s, "* bench.tags: {}", d.tags.join(", "));
    let _ = writeln!(s, "* bench.timing: no");
    let _ = writeln!(s, "* bench.note: {}", d.note);
    s.push_str(&d.body);
    GeneratedDeck {
        id: d.id,
        family: d.family.to_string(),
        size: d.size,
        est_rss_mib: 16,
        deck: s,
    }
}

fn node(i: usize) -> String {
    if i == 0 {
        "0".to_string()
    } else {
        format!("n{i}")
    }
}

/// `k` distinct values of `0..=n` (`k <= n+1`), partial Fisher-Yates.
fn distinct(r: &mut Rng, lo: usize, hi: usize, k: usize) -> Vec<usize> {
    let mut v: Vec<usize> = (lo..=hi).collect();
    for i in 0..k.min(v.len()) {
        let j = i + r.below(v.len() - i);
        v.swap(i, j);
    }
    v.truncate(k);
    v
}

fn pm(r: &mut Rng) -> f64 {
    if r.below(2) == 0 {
        1.0
    } else {
        -1.0
    }
}

// ------------------------------------------------------------------------------------------------ fuzz_linear

fn linear_body(r: &mut Rng) -> (String, usize, usize) {
    let n = 3 + r.below(10);
    let mut b = String::new();
    let mut ri = 0;
    for i in 1..=n {
        let p = r.below(i);
        ri += 1;
        let _ = writeln!(
            b,
            "R{ri} {} {} {}",
            node(i),
            node(p),
            e5(r.log_range(1e1, 1e7))
        );
    }
    for _ in 0..r.below(n + 1) {
        let d = distinct(r, 0, n, 2);
        ri += 1;
        let _ = writeln!(
            b,
            "R{ri} {} {} {}",
            node(d[0]),
            node(d[1]),
            e5(r.log_range(1e1, 1e7))
        );
    }
    let nv = 1 + r.below(3).min(n - 1);
    for (k, nd) in distinct(r, 1, n, nv).into_iter().enumerate() {
        let _ = writeln!(
            b,
            "V{} {} 0 {}",
            k + 1,
            node(nd),
            e5(pm(r) * r.range(0.5, 20.0))
        );
    }
    for k in 0..r.below(3) {
        let d = distinct(r, 0, n, 2);
        let _ = writeln!(
            b,
            "I{} {} {} {}",
            k + 1,
            node(d[0]),
            node(d[1]),
            e5(pm(r) * r.log_range(1e-7, 1e-4))
        );
    }
    for k in 0..r.below(3) {
        let d = distinct(r, 0, n, 2);
        let c = distinct(r, 0, n, 2);
        let _ = writeln!(
            b,
            "G{} {} {} {} {} {}",
            k + 1,
            node(d[0]),
            node(d[1]),
            node(c[0]),
            node(c[1]),
            e5(pm(r) * r.log_range(1e-6, 1e-3))
        );
    }
    b.push_str(".op\n.end\n");
    (b, n, nv)
}

fn probes_v(n: usize, nv: usize) -> Vec<String> {
    let mut p: Vec<String> = (1..=n).map(|i| format!("v(n{i})")).collect();
    for k in 1..=nv {
        p.push(format!("i(v{k})"));
    }
    p
}

/// Oracle solution check that also returns the node voltages (`v(...)` probes) for the clamp predicate.
fn solve_probes(deck: &str, spec: &str, probes: &[String], vmax: f64) -> Option<Vec<f64>> {
    let mut volts = Vec::new();
    for p in probes {
        let v = oracle::evaluate(deck, spec, p, &[0.0]).ok()?;
        if !v
            .iter()
            .all(|x| x.is_finite() && (!p.starts_with("v(") || x.abs() <= vmax))
        {
            return None;
        }
        if p.starts_with("v(") {
            volts.push(v[0]);
        }
    }
    Some(volts)
}

/// RustSpice clamps Newton iterates to the range spanned by the DC source values (audit a3 F02): a deck is
/// `clamp_sensitive` when its exact solution has a node outside `[min(0, Vdc), max(0, Vdc)]`.
fn clamp_sensitive(body: &str, volts: &[f64]) -> bool {
    let (mut lo, mut hi) = (0.0f64, 0.0f64);
    for l in body.lines() {
        let t: Vec<&str> = l.split_whitespace().collect();
        if t.len() == 4 && t[0].starts_with('V') {
            if let Ok(v) = t[3].parse::<f64>() {
                lo = lo.min(v);
                hi = hi.max(v);
            }
        }
    }
    volts.iter().any(|&v| v < lo - 1e-9 || v > hi + 1e-9)
}

fn all_probes_ok(deck: &str, spec: &str, probes: &[String], vmax: f64) -> bool {
    probes
        .iter()
        .all(|p| match oracle::evaluate(deck, spec, p, &[0.0]) {
            Ok(v) => v
                .iter()
                .all(|x| x.is_finite() && (!p.starts_with("v(") || x.abs() <= vmax)),
            Err(_) => false,
        })
}

/// Largest 1-norm condition number of the MNA matrix of a `fuzz_linear` deck: the relative error of a backward-stable
/// solver is below `cond * 1.1e-16 = 1.1e-8` there, two orders under the 1e-6 tolerance, so a mismatch can only be a
/// defect (calibration over 400 generated decks: median 1.2e5, 99th percentile 3.6e7, maximum 1.5e9).
const LINEAR_COND_MAX: f64 = 1e8;

fn gen_linear(seed: u64, k: usize) -> GeneratedDeck {
    let s = sub_seed(seed, 1, k as u64);
    let mut r = Rng::new(s);
    let title = format!("fuzz_linear seed={s}");
    let mut chosen = None;
    for _ in 0..40 {
        let (body, n, nv) = linear_body(&mut r);
        let probes = probes_v(n, nv);
        let deck = format!("{title}\n{body}");
        if !all_probes_ok(&deck, "dc_linear", &probes, 1e4) {
            continue;
        }
        if let Ok(c) = oracle::dc_condition(&deck) {
            if c <= LINEAR_COND_MAX {
                chosen = Some((body, n, nv, probes, c));
                break;
            }
        }
    }
    let (body, n, nv, probes, cond) = chosen.unwrap_or_else(|| {
        (
            "R1 n1 0 1e3\nR2 n2 n1 2.2e3\nR3 n3 n2 4.7e3\nV1 n3 0 5\n.op\n.end\n".to_string(),
            3,
            1,
            probes_v(3, 1),
            f64::NAN,
        )
    });
    render(FuzzDeck {
        id: format!("fuzz_linear_{k:03}"),
        family: "fuzz_linear",
        probes,
        tol_rel: Some(1e-6),
        tol_abs: Some(1e-9),
        tol_norm: None,
        floor: None,
        oracle: Some("dc_linear"),
        size: (n + nv) as u64,
        tags: vec!["fuzz".into(), "family:fuzz_linear".into(), "linear".into(), format!("seed:{s}")],
        gap: None,
        note: format!("random connected resistor network, R over six decades, V/I/G sources; per-deck seed {s}; oracle-validated; MNA cond1 {cond:.2e} (bound {LINEAR_COND_MAX:.0e})"),
        title,
        body,
    })
}

// ------------------------------------------------------------------------------------------------ fuzz_diode

fn diode_body(r: &mut Rng) -> (String, usize, usize, bool) {
    let n = 3 + r.below(6);
    let mut b = String::new();
    // models first
    let nd = 1 + r.below(4);
    let mut models = Vec::new();
    for k in 0..nd {
        let is = r.log_range(1e-16, 1e-12);
        let nf = [1.0, 1.2, 1.5][r.below(3)];
        models.push(format!("DM{k}"));
        let _ = writeln!(b, ".model DM{k} D(IS={} N={})", e5(is), num(nf));
    }
    let mut ri = 0;
    for i in 1..=n {
        let p = r.below(i);
        ri += 1;
        let _ = writeln!(
            b,
            "R{ri} {} {} {}",
            node(i),
            node(p),
            e5(r.log_range(1e2, 1e5))
        );
    }
    // diodes behind series resistors
    let mut extra = n; // next free node index
    for k in 0..nd {
        let d = distinct(r, 0, n, 2);
        extra += 1;
        ri += 1;
        let (a, c) = if r.below(2) == 0 {
            (d[0], d[1])
        } else {
            (d[1], d[0])
        };
        let _ = writeln!(b, "D{} {} n{extra} {}", k + 1, node(a), models[k]);
        let _ = writeln!(
            b,
            "R{ri} n{extra} {} {}",
            node(c),
            e5(r.log_range(1e1, 1e5))
        );
    }
    // sources behind series resistors
    let ns = 1 + r.below(2);
    let srcn = distinct(r, 1, n, ns.min(n));
    let mut nsrc = 0;
    let mut v1node = 0;
    for (k, nd_) in srcn.into_iter().enumerate() {
        extra += 1;
        ri += 1;
        let _ = writeln!(
            b,
            "V{} n{extra} 0 {}",
            k + 1,
            e5(pm(r) * r.range(0.5, 12.0))
        );
        let _ = writeln!(
            b,
            "R{ri} n{extra} {} {}",
            node(nd_),
            e5(r.log_range(1e1, 1e4))
        );
        if k == 0 {
            v1node = extra;
        }
        nsrc += 1;
    }
    let mut stacked = false;
    if r.below(10) < 3 {
        // V2 sits on top of V1: its negative terminal is V1's node, so that node n_top = V1 + V2 exceeds every DC source
        stacked = true;
        extra += 1;
        ri += 1;
        nsrc += 1;
        let _ = writeln!(b, "V{nsrc} n{extra} n{v1node} {}", e5(r.range(0.5, 8.0)));
        let tgt = 1 + r.below(n);
        let _ = writeln!(
            b,
            "R{ri} n{extra} {} {}",
            node(tgt),
            e5(r.log_range(1e1, 1e4))
        );
    }
    b.push_str(".op\n.end\n");
    (b, extra, nsrc, stacked)
}

fn gen_diode(seed: u64, k: usize) -> GeneratedDeck {
    let s = sub_seed(seed, 2, k as u64);
    let mut r = Rng::new(s);
    let title = format!("fuzz_diode seed={s}");
    let mut chosen = None;
    for _ in 0..40 {
        let (body, n, nv, stacked) = diode_body(&mut r);
        let probes = probes_v(n, nv);
        let deck = format!("{title}\n{body}");
        if let Some(volts) = solve_probes(&deck, "dc_nonlinear", &probes, 1e3) {
            let clamp = clamp_sensitive(&body, &volts);
            chosen = Some((body, n, nv, stacked, clamp, probes));
            break;
        }
    }
    let (body, n, nv, stacked, clamp, probes) = chosen.unwrap_or_else(|| {
        (
            ".model DM0 D(IS=1e-14 N=1)\nR1 n1 0 1e3\nV1 n2 0 5\nR2 n2 n3 1e3\nD1 n3 n1 DM0\n.op\n.end\n".to_string(),
            3,
            1,
            false,
            false,
            probes_v(3, 1),
        )
    });
    let mut tags = vec![
        "fuzz".into(),
        "family:fuzz_diode".into(),
        "nonlinear".into(),
        format!("seed:{s}"),
    ];
    if stacked {
        tags.push("stacked_source".into());
    }
    if clamp {
        tags.push("clamp_sensitive".into());
    }
    render(FuzzDeck {
        id: format!("fuzz_diode_{k:03}"),
        family: "fuzz_diode",
        probes,
        tol_rel: Some(1e-6),
        tol_abs: Some(1e-9),
        tol_norm: None,
        floor: None,
        oracle: Some("dc_nonlinear"),
        size: (n + nv) as u64,
        tags,
        gap: None,
        note: format!(
            "random diode-resistor network (IS 1e-16..1e-12), per-deck seed {s}{}; oracle-validated",
            match (stacked, clamp) {
                (true, true) => "; stacked source, a node lies outside the DC-source range (clamp-sensitive)",
                (true, false) => "; stacked source, solution inside the DC-source range",
                (false, true) => "; a node lies outside the DC-source range (clamp-sensitive)",
                _ => "",
            }
        ),
        title,
        body,
    })
}

// ------------------------------------------------------------------------------------------------ fuzz_rc

fn gen_rc(seed: u64, k: usize) -> GeneratedDeck {
    let s = sub_seed(seed, 3, k as u64);
    let mut r = Rng::new(s);
    let title = format!("fuzz_rc seed={s}");
    let n = 3 + r.below(6);
    let tref = 10f64.powf(r.range(-8.0, -6.0));
    // decimal-clean step: every time below is an integer multiple of `h`, so source edges sit on RustSpice's fixed
    // grid and ngspice's breakpoints coincide with its output grid. h = Tref/64 (it was Tref/40: over 400 decks the
    // fixed-step trapezoid then reached 1.3 % of swing against the 2 % tolerance, with a heavy tail; at Tref/64 the
    // error scales with h^2 and the worst case is ~0.5 %, so a failure is a defect, not discretisation)
    let h: f64 = format!("{:.5e}", tref / 64.0)
        .parse()
        .unwrap_or(tref / 64.0);
    let t9 = |steps: usize| format!("{:.9e}", steps as f64 * h);
    let mut b = String::new();
    let mut rpar = vec![1e4; n + 1];
    // resistor tree rooted at n1 (one resistor to ground), so every node is connected to the source
    for i in 1..=n {
        let p = if i == 1 { 0 } else { 1 + r.below(i - 1) };
        let rv = r.log_range(1e3, 1e4);
        rpar[i] = rv;
        let _ = writeln!(b, "R{i} {} {} {}", node(i), node(p), e5(rv));
    }
    for j in 0..r.below(3) {
        let nd = 1 + r.below(n);
        let _ = writeln!(b, "Rg{j} {} 0 {}", node(nd), e5(r.log_range(1e3, 1e5)));
    }
    for i in 1..=n {
        let c = tref * 10f64.powf(r.range(-0.5, 0.5)) / rpar[i];
        let _ = writeln!(b, "C{i} {} 0 {}", node(i), e5(c));
    }
    for j in 0..r.below(3) {
        let d = distinct(&mut r, 1, n, 2);
        let c = tref * 10f64.powf(r.range(-0.5, 0.5)) / rpar[d[0]];
        let _ = writeln!(b, "Cx{j} {} {} {}", node(d[0]), node(d[1]), e5(c));
    }
    let src = 1 + r.below(n);
    let v2 = r.range(0.5, 5.0);
    let td = r.below(33);
    let tr = 6 + r.below(8);
    let tf = 6 + r.below(8);
    let pw = 64 + r.below(193);
    let per = tr + pw + tf + 32 + r.below(97);
    let _ = writeln!(
        b,
        "V1 vs 0 PULSE(0 {} {} {} {} {} {})",
        e5(v2),
        t9(td),
        t9(tr),
        t9(tf),
        t9(pw),
        t9(per)
    );
    let _ = writeln!(b, "Rs vs {} {}", node(src), e5(r.log_range(1e3, 5e3)));
    let tstop_steps = 512usize.max(per * 3 / 2);
    let _ = writeln!(b, ".tran {} {}", e5(h), t9(tstop_steps));
    b.push_str(".end\n");
    render(FuzzDeck {
        id: format!("fuzz_rc_{k:03}"),
        family: "fuzz_rc",
        probes: (1..=n).map(|i| format!("v(n{i})")).collect(),
        tol_rel: None,
        tol_abs: None,
        tol_norm: Some(2e-2),
        floor: Some(1e-3),
        oracle: Some("tran_lti"),
        size: (n + 2) as u64,
        tags: vec!["fuzz".into(), "family:fuzz_rc".into(), "linear".into(), "tran".into(), format!("seed:{s}")],
        gap: None,
        note: format!("random RC network (tree rooted at n1), Tref = {tref:.3e} s, step Tref/64, PULSE edges on the step grid, behind Rs; per-deck seed {s}"),
        title,
        body: b,
    })
}

// ------------------------------------------------------------------------------------------------ fuzz_cmos

/// Logic level of a CMOS signal for the vacuity filter: `Zero` below the NMOS threshold (0.7 V), `One` above
/// `VDD - |VTOP|` = 2.6 V, `Mid` in between (both devices of a gate conduct: analog level).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Lv {
    Zero,
    One,
    Mid,
}

fn level_of(v: f64) -> Lv {
    if v < 0.7 {
        Lv::Zero
    } else if v > 2.6 {
        Lv::One
    } else {
        Lv::Mid
    }
}

/// Random feed-forward gate network; returns the body, the gate output names and whether at least one output is a
/// clean logic one (about 3.3 V): without such an output every probe can sit at 0 V, which the engine rightly refuses
/// to compare (`unverified`: all probes vacuous; 5 of 100 decks per seed before this filter).
fn cmos_body(r: &mut Rng) -> (String, Vec<String>, usize, usize, bool) {
    let mut b = String::new();
    b.push_str(".model NMOD NMOS(LEVEL=1 VTO=0.7 KP=120u LAMBDA=0.02)\n.model PMOD PMOS(LEVEL=1 VTO=-0.7 KP=60u LAMBDA=0.02)\nVDD vdd 0 DC 3.3\n");
    let nin = 3 + r.below(2);
    let mut signals: Vec<String> = Vec::new();
    let mut lv: Vec<Lv> = Vec::new();
    for i in 0..nin {
        let level = match r.below(3) {
            0 => 0.0,
            1 => 3.3,
            _ => r.range(0.6, 2.7),
        };
        let _ = writeln!(b, "Vin{i} in{i} 0 DC {}", e5(level));
        signals.push(format!("in{i}"));
        lv.push(level_of(level));
    }
    let ng = 2 + r.below(5);
    let mut outs = Vec::new();
    for g in 0..ng {
        let out = format!("o{g}");
        let kind = r.below(3);
        let ia = r.below(signals.len());
        let ic = r.below(signals.len());
        let (a, c) = (signals[ia].clone(), signals[ic].clone());
        let (la, lc) = (lv[ia], lv[ic]);
        let level = match kind {
            0 => {
                let _ = writeln!(b, "MP{g}a {out} {a} vdd vdd PMOD W=20u L=1u");
                let _ = writeln!(b, "MN{g}a {out} {a} 0 0 NMOD W=10u L=1u");
                match la {
                    Lv::Zero => Lv::One,
                    Lv::One => Lv::Zero,
                    Lv::Mid => Lv::Mid,
                }
            }
            1 => {
                let _ = writeln!(b, "MP{g}a {out} {a} vdd vdd PMOD W=20u L=1u");
                let _ = writeln!(b, "MP{g}b {out} {c} vdd vdd PMOD W=20u L=1u");
                let _ = writeln!(b, "MN{g}a {out} {a} x{g} 0 NMOD W=20u L=1u");
                let _ = writeln!(b, "MN{g}b x{g} {c} 0 0 NMOD W=20u L=1u");
                // NAND: any low input pulls high, both high pulls low
                if la == Lv::Zero || lc == Lv::Zero {
                    Lv::One
                } else if la == Lv::One && lc == Lv::One {
                    Lv::Zero
                } else {
                    Lv::Mid
                }
            }
            _ => {
                let _ = writeln!(b, "MP{g}a y{g} {a} vdd vdd PMOD W=40u L=1u");
                let _ = writeln!(b, "MP{g}b {out} {c} y{g} vdd PMOD W=40u L=1u");
                let _ = writeln!(b, "MN{g}a {out} {a} 0 0 NMOD W=10u L=1u");
                let _ = writeln!(b, "MN{g}b {out} {c} 0 0 NMOD W=10u L=1u");
                // NOR: any high input pulls low, both low pulls high
                if la == Lv::One || lc == Lv::One {
                    Lv::Zero
                } else if la == Lv::Zero && lc == Lv::Zero {
                    Lv::One
                } else {
                    Lv::Mid
                }
            }
        };
        outs.push(out.clone());
        signals.push(out);
        lv.push(level);
    }
    b.push_str(".op\n.end\n");
    let high = lv[nin..].contains(&Lv::One);
    (b, outs, ng, nin, high)
}

fn gen_cmos(seed: u64, k: usize) -> GeneratedDeck {
    let s = sub_seed(seed, 4, k as u64);
    let mut r = Rng::new(s);
    let title = format!("fuzz_cmos seed={s}");
    let mut chosen = None;
    for _ in 0..40 {
        let c = cmos_body(&mut r);
        if c.4 {
            chosen = Some(c);
            break;
        }
    }
    let (b, outs, ng, nin, _) = chosen.unwrap_or_else(|| {
        (
            ".model NMOD NMOS(LEVEL=1 VTO=0.7 KP=120u LAMBDA=0.02)\n.model PMOD PMOS(LEVEL=1 VTO=-0.7 KP=60u LAMBDA=0.02)\nVDD vdd 0 DC 3.3\nVin0 in0 0 DC 0\nMP0a o0 in0 vdd vdd PMOD W=20u L=1u\nMN0a o0 in0 0 0 NMOD W=10u L=1u\n.op\n.end\n".to_string(),
            vec!["o0".to_string()],
            1,
            1,
            true,
        )
    });
    render(FuzzDeck {
        id: format!("fuzz_cmos_{k:03}"),
        family: "fuzz_cmos",
        probes: outs.iter().map(|o| format!("v({o})")).collect(),
        tol_rel: Some(1e-6),
        tol_abs: Some(1e-8),
        tol_norm: None,
        floor: None,
        oracle: None,
        size: (ng * 2 + nin + 2) as u64,
        tags: vec!["fuzz".into(), "family:fuzz_cmos".into(), "nonlinear".into(), "mos".into(), format!("seed:{s}")],
        gap: None,
        note: format!("random feed-forward CMOS gate network (INV/NAND2/NOR2), inputs 0, 3.3 V or intermediate; at least one gate output is a clean logic one (non-vacuous probe); per-deck seed {s}"),
        title,
        body: b,
    })
}

/// Valid random decks (linear resistor networks, diode-resistor, RC, CMOS gate networks); `count` decks PER fuzz
/// family, ids `fuzz_<family>_<k>`. Deterministic in `(count, seed)`; deck `k` does not depend on `count`.
pub fn fuzz_decks(count: usize, seed: u64) -> Vec<GeneratedDeck> {
    let mut out = Vec::with_capacity(4 * count);
    for k in 0..count {
        out.push(gen_linear(seed, k));
    }
    for k in 0..count {
        out.push(gen_diode(seed, k));
    }
    for k in 0..count {
        out.push(gen_rc(seed, k));
    }
    for k in 0..count {
        out.push(gen_cmos(seed, k));
    }
    out
}

// ------------------------------------------------------------------------------------------------ mutator

const BUILTIN_CORPUS: [(&str, &str); 3] = [
    ("builtin_rc", "rc low-pass\nV1 in 0 PULSE(0 1 1n 1n 1n 20n 50n)\nR1 in out 1k\nC1 out 0 1n\n.tran 1n 100n\n.end\n"),
    ("builtin_div", "divider\nV1 in 0 DC 5\nR1 in mid 1k\nR2 mid 0 2k\n.op\n.end\n"),
    (
        "builtin_diode",
        "diode\n.model DMOD D(IS=1e-14 N=1)\nV1 in 0 DC 5\nR1 in a 1k\nD1 a 0 DMOD\n.subckt cell x y\nR1 x y 1k\n.ends cell\nX1 a b cell\nR2 b 0 1k\n.dc V1 0 5 0.5\n.end\n",
    ),
];

fn sanitize_id(s: &str) -> String {
    let t: String = s
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    t.trim_matches('_').to_string()
}

fn is_numeric_token(t: &str) -> bool {
    let b = t.as_bytes();
    if b.is_empty() {
        return false;
    }
    let first = if b[0] == b'-' || b[0] == b'+' {
        b.get(1).copied()
    } else {
        Some(b[0])
    };
    matches!(first, Some(c) if c.is_ascii_digit() || c == b'.') && oracle::parse_number(t).is_some()
}

/// Lines of the source deck without its `* bench.*` metadata.
fn strip_meta(deck: &str) -> Vec<String> {
    deck.lines()
        .filter(|l| !l.trim_start().starts_with("* bench."))
        .map(|l| l.to_string())
        .collect()
}

fn first_vsource(lines: &[String]) -> Option<String> {
    lines
        .iter()
        .find(|l| l.starts_with('V') || l.starts_with('v'))
        .and_then(|l| l.split_whitespace().next().map(|s| s.to_string()))
}

fn replace_number(
    lines: &mut [String],
    r: &mut Rng,
    with: &dyn Fn(&mut Rng) -> String,
) -> Option<(usize, usize)> {
    // candidate (line, token) positions: all numeric tokens outside comments and `.model`/title
    let mut cands: Vec<(usize, usize)> = Vec::new();
    for (li, l) in lines.iter().enumerate().skip(1) {
        let t = l.trim_start();
        if t.starts_with('*') || t.to_ascii_lowercase().starts_with(".model") {
            continue;
        }
        for (ti, tok) in l.split_whitespace().enumerate() {
            if ti > 0 && is_numeric_token(tok) {
                cands.push((li, ti));
            }
        }
    }
    if cands.is_empty() {
        return None;
    }
    let (li, ti) = cands[r.below(cands.len())];
    let rep = with(r);
    let toks: Vec<String> = lines[li]
        .split_whitespace()
        .enumerate()
        .map(|(i, t)| if i == ti { rep.clone() } else { t.to_string() })
        .collect();
    lines[li] = toks.join(" ").trim_end().to_string();
    Some((li, ti))
}

/// Whether a huge number at token `ti` of line `li` is a documented resource bomb of RustSpice (measured with the
/// shipped binary, 1 GiB cap, < 2 s to reach it): `.dc` start / stop of 1e300, 1e999 or 400 digits and the step when it
/// overflows to infinity (`1e999`; the literal `inf` and `nan` are refused), and `.tran` tstop of 1e300 or more
/// (`tstep` is harmless: `.tran 1e300 50n` runs).
fn sweep_bomb(lines: &[String], (li, ti): (usize, usize), nonfinite: bool) -> bool {
    let card = lines[li]
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    match card.as_str() {
        ".dc" => ti == 2 || ti == 3 || (ti == 4 && nonfinite),
        ".tran" => ti == 2,
        _ => false,
    }
}

/// Sources RustSpice can parse and solve today (no device card it refuses, not an expected refusal, no known gap): only on these can the
/// resource-bomb mutations (`bigtran`, `bigdc`, `recsub`) reach the bomb instead of being refused earlier.
fn bomb_eligible(text: &str) -> bool {
    let refused_device = text.lines().skip(1).any(|l| {
        let t = l.trim_start();
        matches!(
            t.chars().next(),
            Some(
                'E' | 'F'
                    | 'H'
                    | 'B'
                    | 'S'
                    | 'W'
                    | 'K'
                    | 'T'
                    | 'O'
                    | 'Q'
                    | 'J'
                    | 'Z'
                    | 'U'
                    | 'e'
                    | 'f'
                    | 'h'
                    | 'b'
                    | 's'
                    | 'w'
                    | 'k'
                    | 't'
                    | 'o'
                    | 'q'
                    | 'j'
                    | 'z'
                    | 'u'
            )
        )
    });
    // a source with a known gap of its own is refused or mis-solved before the sweep/transient starts (measured: the
    // `clamp-rail` sources acc_dc_clamp_isrc_node_diode and rob_cv_clamp_vccs_50v_node fail Newton in 1.8 ms), so a
    // bomb tag on its mutants would be a false `gap_closed`
    !refused_device
        && !text.contains("bench.expect: loud_error")
        && !text.contains("bench.known_gap:")
        && !refused_by_design(text)
}

/// Tags of decks that exist to be malformed, singular or mis-parsed (`expect: either` robustness probes where
/// RustSpice refuses by design: R card without a value, an inductor across a source, a title line that is a dot card).
/// Seeds 1 and 2 of the 100-per-family run produced three false `gap_closed` verdicts from exactly such sources: the
/// refusal (`ParseError`, `SingularMatrix`, `TopologyError`) comes before the sweep or transient is reached, so the
/// bomb is never exercised and the gap is not closed.
const REFUSED_BY_DESIGN_TAGS: [&str; 5] = ["hostile", "singular", "syntax", "parser", "file"];

fn refused_by_design(text: &str) -> bool {
    text.lines().any(|l| {
        let t = l.trim_start().trim_start_matches('*').trim_start();
        t.strip_prefix("bench.tags:").is_some_and(|rest| {
            rest.split(',')
                .any(|tag| REFUSED_BY_DESIGN_TAGS.contains(&tag.trim()))
        })
    })
}

/// A static deck that is itself a documented resource bomb (`known_gap: resource-budget`): mutating it further would
/// only rediscover the same bomb under another kind.
fn is_resource_bomb_source(text: &str) -> bool {
    text.contains("bench.known_gap: resource-budget")
}

fn insert_before_end(lines: &mut Vec<String>, extra: &[&str]) {
    let pos = lines
        .iter()
        .rposition(|l| l.trim().eq_ignore_ascii_case(".end"))
        .unwrap_or(lines.len());
    for (k, e) in extra.iter().enumerate() {
        lines.insert(pos + k, (*e).to_string());
    }
}

/// Returns the mutated deck and whether the mutation hit a documented resource bomb at a number position
/// ([`sweep_bomb`]); the three structural bomb kinds are tagged by kind in [`mutants`].
fn mutate(kind: &str, src: &str, r: &mut Rng) -> (String, bool) {
    let mut bomb = false;
    let mut lines = strip_meta(src);
    if lines.is_empty() {
        lines.push("empty".into());
    }
    let n = lines.len();
    let mut text_override: Option<String> = None;
    match kind {
        "dellin" => {
            if n > 1 {
                lines.remove(1 + r.below(n - 1));
            }
        }
        "duplin" => {
            let i = r.below(n);
            let l = lines[i].clone();
            lines.insert(i, l);
        }
        "trunc" => {
            let full = lines.join("\n");
            let cut = (r.next_f64() * full.len() as f64) as usize;
            let mut c = cut.min(full.len());
            while c > 0 && !full.is_char_boundary(c) {
                c -= 1;
            }
            text_override = Some(full[..c].to_string());
        }
        "numnan" => {
            if replace_number(&mut lines, r, &|_| "nan".into()).is_none() {
                lines.insert(n, "R1 a b nan".into());
            }
        }
        "numinf" => {
            if replace_number(&mut lines, r, &|_| "inf".into()).is_none() {
                lines.insert(n, "R1 a b inf".into());
            }
        }
        "num1e999" => match replace_number(&mut lines, r, &|_| "1e999".into()) {
            Some(pos) => bomb = sweep_bomb(&lines, pos, true),
            None => lines.insert(n, "R1 a b 1e999".into()),
        },
        "numneg" => {
            replace_number(&mut lines, r, &|rr| format!("-{}", 1 + rr.below(9)));
        }
        "numhuge" => {
            if let Some(pos) = replace_number(&mut lines, r, &|_| "1e300".into()) {
                bomb = sweep_bomb(&lines, pos, false);
            }
        }
        "numempty" => {
            replace_number(&mut lines, r, &|_| String::new());
        }
        "numzero" => {
            replace_number(&mut lines, r, &|_| "0".into());
        }
        "longnum" => {
            if let Some(pos) = replace_number(&mut lines, r, &|_| format!("1{}", "0".repeat(400))) {
                bomb = sweep_bomb(&lines, pos, true);
            }
        }
        "dropend" => lines.retain(|l| !l.trim().eq_ignore_ascii_case(".end")),
        "dupend" => {
            insert_before_end(&mut lines, &[".end"]);
            lines.push("R9999 zz 0 1k".into());
            lines.push(".end".into());
        }
        "dropends" => lines.retain(|l| !l.trim().to_ascii_lowercase().starts_with(".ends")),
        "dupends" => {
            insert_before_end(&mut lines, &[".ends", ".ends"]);
        }
        "unbal" => insert_before_end(&mut lines, &[".subckt bogus a b", "R1 a b 1k"]),
        "recsub" => insert_before_end(
            &mut lines,
            &[
                ".subckt loop a b",
                "X1 a b loop",
                ".ends loop",
                "Xtop n1 0 loop",
            ],
        ),
        "deepsub" => {
            let mut extra: Vec<String> = Vec::new();
            for d in 0..200 {
                extra.push(format!(".subckt lvl{d} a b"));
                if d == 0 {
                    extra.push("R1 a b 1k".into());
                } else {
                    extra.push(format!("X1 a b lvl{}", d - 1));
                }
                extra.push(format!(".ends lvl{d}"));
            }
            extra.push("Xdeep dd 0 lvl199".into());
            extra.push("Rdd dd 0 1k".into());
            let refs: Vec<&str> = extra.iter().map(|s| s.as_str()).collect();
            insert_before_end(&mut lines, &refs);
        }
        "bigtran" => {
            let had = lines
                .iter()
                .any(|l| l.trim_start().to_ascii_lowercase().starts_with(".tran"));
            for l in lines.iter_mut() {
                if l.trim_start().to_ascii_lowercase().starts_with(".tran") {
                    *l = ".tran 1p 1".to_string();
                }
            }
            if !had {
                insert_before_end(
                    &mut lines,
                    &[
                        "V9999 zz 0 PULSE(0 1 0 1n 1n 5n 10n)",
                        "R9999 zz 0 1k",
                        ".tran 1p 1",
                    ],
                );
            }
        }
        "bigdc" => {
            let vs = first_vsource(&lines);
            lines.retain(|l| !l.trim_start().to_ascii_lowercase().starts_with(".dc"));
            match vs {
                Some(v) => insert_before_end(&mut lines, &[&format!(".dc {v} 0 1 1e-9")]),
                None => insert_before_end(
                    &mut lines,
                    &["V9999 zz 0 0", "R9999 zz 0 1k", ".dc V9999 0 1 1e-9"],
                ),
            }
        }
        "longline" => {
            let name = "a".repeat(100_000);
            insert_before_end(&mut lines, &[&format!("R9999 {name} 0 1k")]);
        }
        "nul" | "badutf" | "unicode" => {
            let ch = match kind {
                "nul" => '\u{0}',
                "badutf" => '\u{FFFD}',
                _ => '\u{e9}',
            };
            let cand: Vec<usize> = (1..n)
                .filter(|&i| {
                    let t = lines[i].trim_start();
                    !t.is_empty() && !t.starts_with('*')
                })
                .collect();
            if let Some(&i) = cand.get(r.below(cand.len().max(1))) {
                let chars: Vec<char> = lines[i].chars().collect();
                let at = r.below(chars.len().max(1));
                let mut s: String = chars[..at].iter().collect();
                s.push(ch);
                s.extend(chars[at..].iter());
                lines[i] = s;
            } else {
                lines.push(format!("R1 a{ch} b 1k"));
            }
        }
        "empty" => lines.truncate(1),
        "notitle" => {
            if let Some(p) = (1..n).find(|&i| {
                let t = lines[i].trim_start();
                !t.is_empty() && !t.starts_with('*') && !t.starts_with('.')
            }) {
                let l = lines.remove(p);
                lines[0] = l;
            } else {
                lines[0] = "R1 a b 1k".to_string();
            }
        }
        "selfres" => insert_before_end(&mut lines, &["Rself a a 1k"]),
        "vloop" => insert_before_end(&mut lines, &["Vl1 q 0 1", "Vl2 q 0 2", "Rq q 0 1k"]),
        "negr" => insert_before_end(&mut lines, &["Rneg a 0 -1k"]),
        "swapcard" => {
            insert_before_end(
                &mut lines,
                &[".op", ".tran 1n 10n", ".dc V1 0 1 0.5", ".op"],
            );
        }
        _ => {}
    }
    let text = match text_override {
        Some(t) => t,
        None => lines.join("\n") + "\n",
    };
    (text, bomb)
}

/// Insert `* bench.*` lines after the first line of `deck` (the title; for a deck that is empty or a single line, a
/// second line is created).
fn with_meta(deck: &str, meta: &str) -> String {
    match deck.find('\n') {
        Some(p) => format!("{}{}{}", &deck[..=p], meta, &deck[p + 1..]),
        None => format!("{deck}\n{meta}"),
    }
}

/// Malformed/hostile variants of `corpus` (`(id, deck text)` pairs): ids `mut_<source>_<kind>_<n>`, metadata
/// `* bench.group: robustness`, `* bench.expect: either`, never-crash semantics (the engine only checks for
/// panic/abort/hang/OOM, comparing with ngspice only for the pass/fail class). `count` mutants in total, kinds cycle
/// through [`MUTATION_KINDS`], sources are drawn from the seeded generator; an empty corpus uses a built-in one.
pub fn mutants(corpus: &[(String, String)], count: usize, seed: u64) -> Vec<GeneratedDeck> {
    let builtin: Vec<(String, String)> = BUILTIN_CORPUS
        .iter()
        .map(|(a, b)| (a.to_string(), b.to_string()))
        .collect();
    let corpus: &[(String, String)] = if corpus.is_empty() { &builtin } else { corpus };
    let mut out = Vec::with_capacity(count);
    for j in 0..count {
        let mut r = Rng::new(sub_seed(seed, 100, j as u64));
        let kind = MUTATION_KINDS[j % MUTATION_KINDS.len()];
        let structural_bomb = matches!(kind, "bigtran" | "bigdc" | "recsub");
        // sources: the structural bombs need a deck RustSpice can parse, every other kind avoids decks that are
        // bombs themselves (a recursive subcircuit stays recursive under any mutation)
        let mut pick = r.below(corpus.len());
        for _ in 0..16 {
            let text = &corpus[pick].1;
            let ok = if structural_bomb {
                bomb_eligible(text)
            } else {
                !is_resource_bomb_source(text)
            };
            if ok {
                break;
            }
            pick = r.below(corpus.len());
        }
        let (sid, text) = &corpus[pick];
        let sid_s = sanitize_id(sid);
        let id = format!("mut_{sid_s}_{kind}_{j}");
        let (body, number_bomb) = mutate(kind, text, &mut r);
        let mut meta = String::new();
        let _ = writeln!(meta, "* bench.id: {id}");
        let _ = writeln!(meta, "* bench.group: robustness");
        let _ = writeln!(meta, "* bench.title: {kind} mutation of {sid_s}");
        let _ = writeln!(meta, "* bench.expect: either");
        let _ = (structural_bomb, number_bomb);
        let _ = writeln!(meta, "* bench.tags: mutant, kind:{kind}, source:{sid_s}");
        let _ = writeln!(meta, "* bench.timing: no");
        let _ = writeln!(meta, "* bench.est_rss_mib: 32");
        let _ = writeln!(meta, "* bench.size: 1");
        let _ = writeln!(meta, "* bench.note: seed={seed} index={j}");
        let deck = with_meta(&body, &meta);
        out.push(GeneratedDeck {
            id,
            family: "mutant".to_string(),
            size: 1,
            est_rss_mib: 32,
            deck,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bench::oracle::testutil;

    fn meta_of(deck: &str, key: &str) -> Option<String> {
        let pat = format!("* bench.{key}:");
        deck.lines()
            .find_map(|l| l.strip_prefix(&pat).map(|v| v.trim().to_string()))
    }

    #[test]
    fn fuzz_is_seed_reproducible_and_seed_sensitive() {
        let a = fuzz_decks(6, 7);
        let b = fuzz_decks(6, 7);
        assert_eq!(a.len(), 24);
        for (x, y) in a.iter().zip(&b) {
            assert_eq!(x.deck, y.deck);
            assert_eq!(x.id, y.id);
        }
        let c = fuzz_decks(6, 8);
        for fam in FUZZ_FAMILIES {
            let xs: Vec<&String> = a
                .iter()
                .filter(|d| d.family == fam)
                .map(|d| &d.deck)
                .collect();
            let ys: Vec<&String> = c
                .iter()
                .filter(|d| d.family == fam)
                .map(|d| &d.deck)
                .collect();
            assert_eq!(xs.len(), 6);
            assert!(
                xs.iter().zip(&ys).all(|(x, y)| x != y),
                "{fam}: a different seed must give different decks"
            );
            // decks within one seed differ from each other
            let mut u = xs.clone();
            u.sort();
            u.dedup();
            assert_eq!(u.len(), 6, "{fam}: decks of one seed must be distinct");
        }
        // prefix property: deck k does not depend on count
        let d3 = fuzz_decks(3, 7);
        for fam in FUZZ_FAMILIES {
            let first_a: Vec<&String> = a
                .iter()
                .filter(|d| d.family == fam)
                .take(3)
                .map(|d| &d.deck)
                .collect();
            let first_3: Vec<&String> = d3
                .iter()
                .filter(|d| d.family == fam)
                .map(|d| &d.deck)
                .collect();
            assert_eq!(first_a, first_3, "{fam}");
        }
    }

    #[test]
    fn fuzz_metadata_and_contracts() {
        let decks = fuzz_decks(25, 1);
        assert_eq!(decks.len(), 100);
        let mut ids = std::collections::BTreeSet::new();
        let mut stacked = 0;
        for d in &decks {
            assert!(ids.insert(d.id.clone()));
            assert_eq!(meta_of(&d.deck, "id").as_deref(), Some(d.id.as_str()));
            assert_eq!(meta_of(&d.deck, "group").as_deref(), Some("robustness"));
            assert_eq!(meta_of(&d.deck, "expect").as_deref(), Some("match"));
            let _tags = meta_of(&d.deck, "tags").unwrap();
            assert_eq!(meta_of(&d.deck, "known_gap"), None, "{}", d.id);
            assert!(meta_of(&d.deck, "tags")
                .unwrap()
                .contains(&format!("family:{}", d.family)));
            assert!(meta_of(&d.deck, "tags").unwrap().contains("seed:"));
            assert!(d.deck.is_ascii() && d.deck.trim_end().ends_with(".end"));
            let lower = d.deck.to_ascii_lowercase();
            let analyses = lower
                .lines()
                .filter(|l| l.starts_with(".op") || l.starts_with(".dc") || l.starts_with(".tran"))
                .count();
            assert_eq!(analyses, 1, "{}", d.id);
            if d.family == "fuzz_rc" {
                assert!(meta_of(&d.deck, "tol.norm").is_some());
                // PULSE edges are never zero
                let l = d
                    .deck
                    .lines()
                    .find(|l| l.starts_with("V1 vs 0 PULSE"))
                    .unwrap();
                let t: Vec<&str> = l.split([' ', '(', ')']).filter(|x| !x.is_empty()).collect();
                let tr: f64 = t[5].parse().unwrap();
                let tf: f64 = t[6].parse().unwrap();
                assert!(tr > 0.0 && tf > 0.0);
            }
            if d.family == "fuzz_diode"
                && meta_of(&d.deck, "tags")
                    .unwrap()
                    .contains("clamp_sensitive")
            {
                stacked += 1;
            }
            // oracle-backed families: every probe evaluates
            if let Some(spec) = meta_of(&d.deck, "oracle") {
                let axis: Vec<f64> = if spec == "tran_lti" {
                    let t: Vec<&str> = lower
                        .lines()
                        .find(|l| l.starts_with(".tran"))
                        .unwrap()
                        .split_whitespace()
                        .collect();
                    let tstop = oracle::parse_number(t[2]).unwrap();
                    (0..=10).map(|i| tstop * i as f64 / 10.0).collect()
                } else {
                    vec![0.0]
                };
                if spec == "tran_lti" && d.id.as_str() > "fuzz_rc_005" {
                    continue; // keep the unit test fast; the ignored cross-check covers all
                }
                for p in meta_of(&d.deck, "probes").unwrap().split(',') {
                    let r = oracle::evaluate(&d.deck, &spec, p.trim(), &axis);
                    assert!(r.is_ok(), "{} {p}: {:?}", d.id, r);
                }
            }
        }
        assert!(
            stacked >= 1,
            "25 diode decks should contain a stacked-source variant"
        );
    }

    #[test]
    fn mutants_are_reproducible_diverse_and_well_formed() {
        let corpus: Vec<(String, String)> = fuzz_decks(2, 1)
            .into_iter()
            .map(|d| (d.id.clone(), d.deck))
            .collect();
        let a = mutants(&corpus, 90, 3);
        let b = mutants(&corpus, 90, 3);
        assert_eq!(a.len(), 90);
        for (x, y) in a.iter().zip(&b) {
            assert_eq!(x.deck, y.deck);
            assert_eq!(x.id, y.id);
        }
        let c = mutants(&corpus, 90, 4);
        assert!(
            a.iter().zip(&c).filter(|(x, y)| x.deck != y.deck).count() > 60,
            "different seed => different mutants"
        );
        let mut ids = std::collections::BTreeSet::new();
        let mut kinds = std::collections::BTreeSet::new();
        for d in &a {
            assert!(ids.insert(d.id.clone()), "duplicate id {}", d.id);
            assert!(
                d.id.starts_with("mut_")
                    && d.id
                        .chars()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'),
                "{}",
                d.id
            );
            assert_eq!(meta_of(&d.deck, "id").as_deref(), Some(d.id.as_str()));
            assert_eq!(meta_of(&d.deck, "group").as_deref(), Some("robustness"));
            assert_eq!(meta_of(&d.deck, "expect").as_deref(), Some("either"));
            assert_eq!(
                d.deck.matches("* bench.id:").count(),
                1,
                "source metadata must be stripped: {}",
                d.id
            );
            let kind = meta_of(&d.deck, "tags")
                .unwrap()
                .split(',')
                .find_map(|t| t.trim().strip_prefix("kind:").map(|k| k.to_string()))
                .unwrap();
            assert!(d.id.contains(&format!("_{kind}_")));
            kinds.insert(kind.clone());
            let gap = meta_of(&d.deck, "known_gap");
            assert!(gap.is_none(), "{}", d.id);
        }
        assert_eq!(
            kinds.len(),
            MUTATION_KINDS.len(),
            "every mutation kind is produced"
        );
        // mutants differ from their sources for the structural kinds
        let by_kind = |k: &str| a.iter().find(|d| d.id.contains(&format!("_{k}_"))).unwrap();
        assert!(by_kind("bigtran").deck.contains(".tran 1p 1"));
        assert!(by_kind("recsub").deck.contains("X1 a b loop"));
        assert!(by_kind("longline").deck.lines().any(|l| l.len() > 100_000));
        assert!(by_kind("nul").deck.contains('\u{0}'));
        assert!(by_kind("badutf").deck.contains('\u{FFFD}'));
        assert!(by_kind("num1e999").deck.contains("1e999"));
        assert!(by_kind("numnan").deck.contains("nan"));
        assert!(by_kind("deepsub").deck.contains(".subckt lvl199"));
        assert!(!by_kind("dropend")
            .deck
            .to_ascii_lowercase()
            .contains("\n.end\n"));
        // empty corpus falls back to the built-in one
        let e = mutants(&[], 5, 1);
        assert_eq!(e.len(), 5);
        assert!(e[0].id.starts_with("mut_builtin_"));
        assert!(mutants(&corpus, 0, 1).is_empty());
    }

    #[test]
    fn mutator_picks_eligible_sources_and_tags_number_bombs_by_position() {
        let corpus = vec![
            (
                "dcdeck".to_string(),
                "t\nV1 in 0 DC 1\nR1 in out 1k\nR2 out 0 1k\n.dc V1 0 5 0.5\n.end\n".to_string(),
            ),
            (
                "trandeck".to_string(),
                "t\nV1 in 0 PULSE(0 1 1n 1n 1n 5n 20n)\nR1 in out 1k\nC1 out 0 1p\n.tran 100p 50n\n.end\n"
                    .to_string(),
            ),
            (
                "refused".to_string(),
                "t\n* bench.expect: loud_error\nV1 in 0 1\nE1 out 0 in 0 2\nR1 out 0 1k\n.op\n.end\n"
                    .to_string(),
            ),
            (
                "bomb".to_string(),
                "t\n* bench.known_gap: resource-budget\nV1 in 0 1\nR1 in 0 1k\n.op\n.end\n".to_string(),
            ),
            (
                // a robustness probe that RustSpice refuses by design (measured: ParseError at the empty R card, so
                // the bomb mutation was never reached and the verdict was a false gap_closed)
                "hostile".to_string(),
                "t\n* bench.expect: either\n* bench.tags: hostile, syntax, parser\nV1 1 0 5\nR1 1 2 1k\nR9 2 0\n.op\n.end\n"
                    .to_string(),
            ),
            (
                // RustSpice's Newton fails on it within 2 ms (measured on the real clamp-rail sources): a bomb tag on
                // a mutant of it was a false gap_closed
                "clamp".to_string(),
                "t\n* bench.known_gap: clamp-rail\nI1 0 a 1m\nD1 a 0 dm\n.model dm D(IS=1e-14)\n.dc I1 0 5m 1m\n.end\n"
                    .to_string(),
            ),
        ];
        let ms = mutants(&corpus, 30 * 40, 5);
        let (mut tagged, mut hit) = (0, 0);
        for d in &ms {
            let kind =
                d.id.split('_')
                    .nth(2)
                    .map(str::to_string)
                    .unwrap_or_default();
            let src = meta_of(&d.deck, "tags")
                .unwrap()
                .split(',')
                .find_map(|t| t.trim().strip_prefix("source:").map(|k| k.to_string()))
                .unwrap();
            if matches!(kind.as_str(), "bigtran" | "bigdc" | "recsub") {
                assert_ne!(
                    src, "refused",
                    "{}: a refused deck cannot reach the bomb",
                    d.id
                );
                assert_ne!(
                    src, "clamp",
                    "{}: a source with a known gap fails before the bomb",
                    d.id
                );
                assert_ne!(
                    src, "hostile",
                    "{}: a source refused by design never reaches the bomb",
                    d.id
                );
            } else {
                assert_ne!(
                    src, "bomb",
                    "{}: a bomb source is not mutated further",
                    d.id
                );
            }
            if matches!(kind.as_str(), "num1e999" | "numhuge" | "longnum") {
                let huge = |t: &str| t == "1e999" || t == "1e300" || t.len() > 300;
                let on_bomb_position = d.deck.lines().any(|l| {
                    let t: Vec<&str> = l.split_whitespace().collect();
                    match t.first().map(|c| c.to_ascii_lowercase()).as_deref() {
                        Some(".dc") => {
                            t.get(2).is_some_and(|x| huge(x))
                                || t.get(3).is_some_and(|x| huge(x))
                                || (kind != "numhuge" && t.get(4).is_some_and(|x| huge(x)))
                        }
                        Some(".tran") => t.get(2).is_some_and(|x| huge(x)),
                        _ => false,
                    }
                });
                let has_tag = meta_of(&d.deck, "known_gap").is_some();
                assert!(!has_tag, "{}", d.id);
                tagged += on_bomb_position as usize;
                hit += 1;
            }
        }
        assert!(hit > 20 && tagged > 0, "{tagged} of {hit}");
    }

    #[test]
    fn refused_by_design_sources_are_recognised_by_tag() {
        assert!(refused_by_design(
            "t\n* bench.tags: hostile, syntax, parser, ngspice_warns\nR1 a 0 1\n"
        ));
        assert!(refused_by_design(
            "t\n*bench.tags:  caught_panic_noise,singular\n"
        ));
        assert!(!refused_by_design(
            "t\n* bench.tags: accuracy, diode, tran\n"
        ));
        assert!(!refused_by_design("t\n* bench.note: hostile syntax\n"));
        assert!(!bomb_eligible(
            "t\n* bench.expect: either\n* bench.tags: hostile\nV1 a 0 1\nR1 a 0 1k\n.tran 1u 1m\n.end\n"
        ));
        assert!(bomb_eligible(
            "t\n* bench.expect: match\n* bench.tags: tran, rc\nV1 a 0 1\nR1 a 0 1k\n.tran 1u 1m\n.end\n"
        ));
    }

    // ----- cross-validation (by hand): ngspice tight vs oracle for >= 20 decks per family, RustSpice probe sample

    fn parse_probe(out: &str) -> Vec<(String, Vec<String>, Vec<Vec<f64>>)> {
        // sections: "# op|dc|tran" then either "name value" lines (op) or a CSV header + rows
        let mut secs: Vec<(String, Vec<String>, Vec<Vec<f64>>)> = Vec::new();
        for l in out.lines() {
            if let Some(k) = l.strip_prefix("# ") {
                secs.push((
                    k.split_whitespace().next().unwrap_or("").to_string(),
                    Vec::new(),
                    Vec::new(),
                ));
            } else if let Some(s) = secs.last_mut() {
                if s.0 == "op" {
                    let mut it = l.split_whitespace();
                    if let (Some(n), Some(v)) = (it.next(), it.next()) {
                        s.1.push(n.to_string());
                        s.2.push(vec![v.parse().unwrap_or(f64::NAN)]);
                    }
                } else if s.1.is_empty() {
                    s.1 = l.split(',').map(|x| x.to_string()).collect();
                } else {
                    s.2.push(
                        l.split(',')
                            .map(|x| x.parse().unwrap_or(f64::NAN))
                            .collect(),
                    );
                }
            }
        }
        secs
    }

    /// `H2_FUZZ_COUNT` decks per family (default 25), `H2_FUZZ_SEED` (default 1). Prints per-family worst ratios
    /// `|error| / tolerance` of (oracle vs tight ngspice, tolerance 1e-9 abs + 1e-8 rel, tran 5e-5 of swing) and
    /// (RustSpice probe vs oracle, or vs ngspice for decks without oracle, deck tolerances) and every violation.
    /// Run: `RSPROBE=... cargo test -p rustspice-cli fuzz::tests::dump_and_check -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn dump_and_check() {
        if !testutil::ngspice_ok() {
            eprintln!("ngspice not available: skipped");
            return;
        }
        let count: usize = std::env::var("H2_FUZZ_COUNT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(25);
        let seed: u64 = std::env::var("H2_FUZZ_SEED")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1);
        let dir = testutil::work_dir().join("fuzz");
        let _ = std::fs::create_dir_all(&dir);
        #[derive(Default)]
        struct Sum {
            decks: usize,
            ng_fail: usize,
            oracle_err: usize,
            rs_fail: usize,
            rs_fail_clamp: usize,
            worst_oracle: f64,
            worst_rs: f64,
            rs_over: usize,
        }
        let mut summary: std::collections::BTreeMap<String, Sum> = Default::default();
        for d in fuzz_decks(count, seed) {
            let path = dir.join(format!("{}.sp", d.id));
            std::fs::write(&path, &d.deck).unwrap();
            let e = summary.entry(d.family.clone()).or_default();
            e.decks += 1;
            let tight = testutil::tight(&d.deck);
            let ng = match testutil::run_ngspice(&tight, &format!("fz_{}", d.id)) {
                Ok(r) => r,
                Err(err) => {
                    eprintln!("{}: ngspice FAILED ({err})", d.id);
                    e.ng_fail += 1;
                    continue;
                }
            };
            let probes: Vec<String> = meta_of(&d.deck, "probes")
                .unwrap()
                .split(',')
                .map(|s| s.trim().to_string())
                .collect();
            let is_tran = d.family == "fuzz_rc";
            let tol_rel: f64 = meta_of(&d.deck, "tol.rel")
                .and_then(|v| v.parse().ok())
                .unwrap_or(1e-6);
            let tol_abs: f64 = meta_of(&d.deck, "tol.abs")
                .and_then(|v| v.parse().ok())
                .unwrap_or(1e-9);
            let floor: f64 = meta_of(&d.deck, "floor")
                .and_then(|v| v.parse().ok())
                .unwrap_or(1e-3);
            let axis = if is_tran {
                ng.cols[0].clone()
            } else {
                vec![0.0]
            };
            let spec = meta_of(&d.deck, "oracle");
            let rs = testutil::run_probe(&path);
            let rs_secs = rs
                .as_ref()
                .filter(|(ok, _, _)| *ok)
                .map(|(_, out, _)| parse_probe(out));
            let clamp = meta_of(&d.deck, "tags")
                .unwrap()
                .contains("clamp_sensitive");
            if let Some((false, _, err)) = &rs {
                eprintln!(
                    "{}: RUSTSPICE FAILED{}: {err}",
                    d.id,
                    if clamp { " [clamp_sensitive]" } else { "" }
                );
                e.rs_fail += 1;
                if clamp {
                    e.rs_fail_clamp += 1;
                }
            }
            for p in &probes {
                let Some(ngv) = ng.col(p) else { continue };
                let truth: Vec<f64> = match &spec {
                    Some(sp) => match oracle::evaluate(&d.deck, sp, p, &axis) {
                        Ok(o) => {
                            let ratio = if is_tran {
                                let sw = ngv.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
                                    - ngv.iter().cloned().fold(f64::INFINITY, f64::min);
                                o.iter()
                                    .zip(ngv)
                                    .fold(0.0f64, |m, (a, b)| m.max((a - b).abs()))
                                    / sw.max(floor)
                                    / 5e-5
                            } else {
                                (o[0] - ngv[0]).abs() / (1e-9 + 1e-8 * ngv[0].abs())
                            };
                            e.worst_oracle = e.worst_oracle.max(ratio);
                            if ratio > 1.0 {
                                eprintln!("{}: {p} oracle vs ngspice ratio {ratio:.2}", d.id);
                            }
                            o
                        }
                        Err(err) => {
                            eprintln!("{}: {p} oracle error {err}", d.id);
                            e.oracle_err += 1;
                            continue;
                        }
                    },
                    None => ngv.clone(),
                };
                if let Some(secs) = &rs_secs {
                    for (kind, hdr, rows) in secs {
                        if kind == "op" {
                            if let Some(i) = hdr.iter().position(|h| h == p) {
                                let ratio = (rows[i][0] - truth[0]).abs()
                                    / (tol_abs + tol_rel * truth[0].abs());
                                e.worst_rs = e.worst_rs.max(ratio);
                                if ratio > 1.0 {
                                    e.rs_over += 1;
                                    eprintln!("{}: {p} RustSpice vs {} ratio {ratio:.2} (rs {:e} ref {:e})", d.id, if spec.is_some() { "oracle" } else { "ngspice" }, rows[i][0], truth[0]);
                                }
                            }
                        } else if kind == "tran" {
                            if let (Some(ci), Some(sp)) =
                                (hdr.iter().position(|h| h == p), spec.as_ref())
                            {
                                let ts: Vec<f64> = rows.iter().map(|r| r[0]).collect();
                                if let Ok(ov) = oracle::evaluate(&d.deck, sp, p, &ts) {
                                    let sw = ov.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
                                        - ov.iter().cloned().fold(f64::INFINITY, f64::min);
                                    let er = rows
                                        .iter()
                                        .zip(&ov)
                                        .fold(0.0f64, |m, (r, o)| m.max((r[ci] - o).abs()))
                                        / sw.max(floor);
                                    let ratio = er
                                        / meta_of(&d.deck, "tol.norm")
                                            .and_then(|v| v.parse::<f64>().ok())
                                            .unwrap_or(2e-2);
                                    e.worst_rs = e.worst_rs.max(ratio);
                                    if ratio > 1.0 {
                                        e.rs_over += 1;
                                        eprintln!("{}: {p} RustSpice(tran) vs oracle norm_err {er:.3e} ratio {ratio:.2}", d.id);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        for (f, v) in &summary {
            eprintln!(
                "{f:12} decks={} ngspice_failed={} oracle_errors={} worst_ratio(oracle vs ngspice)={:.3} worst_ratio(rustspice)={:.3} rustspice_probes_over_tol={} rustspice_run_failures={} (clamp_sensitive: {})",
                v.decks, v.ng_fail, v.oracle_err, v.worst_oracle, v.worst_rs, v.rs_over, v.rs_fail, v.rs_fail_clamp
            );
        }
    }

    /// Writes the fuzz decks of `F1_SEED` (default 1) x `F1_COUNT` (default 100) per family to `F1_DIR` (one `<id>.sp`
    /// each), without running anything, for external calibration.
    #[test]
    #[ignore]
    fn dump_fuzz_decks() {
        let seed: u64 = std::env::var("F1_SEED")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1);
        let count: usize = std::env::var("F1_COUNT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(100);
        let dir = std::path::PathBuf::from(std::env::var("F1_DIR").expect("F1_DIR"));
        std::fs::create_dir_all(&dir).unwrap();
        for d in fuzz_decks(count, seed) {
            std::fs::write(dir.join(format!("{}.sp", d.id)), &d.deck).unwrap();
        }
    }

    /// Runs `H2_MUT_COUNT` (default 60) mutants of the Small family corpus through ngspice and the RustSpice probe and
    /// prints the exit classification of each (informational).
    #[test]
    #[ignore]
    fn dump_mutants() {
        let count: usize = std::env::var("H2_MUT_COUNT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(60);
        let corpus: Vec<(String, String)> =
            crate::bench::families::generate(crate::bench::types::Scale::Small, 1)
                .into_iter()
                .filter(|d| d.deck.len() < 200_000)
                .map(|d| (d.id, d.deck))
                .collect();
        let dir = testutil::work_dir().join("mut");
        let _ = std::fs::create_dir_all(&dir);
        for d in mutants(&corpus, count, 1) {
            let path = dir.join(format!("{}.sp", d.id));
            std::fs::write(&path, d.deck.as_bytes()).unwrap();
            let ng = if testutil::ngspice_ok() {
                let out = testutil::guarded(30, 4)
                    .arg("ngspice")
                    .arg("-b")
                    .arg(&path)
                    .output()
                    .ok();
                out.map(|o| format!("ngspice rc={:?}", o.status.code()))
                    .unwrap_or_default()
            } else {
                "ngspice n/a".into()
            };
            let rs = match testutil::run_probe(&path) {
                Some((ok, _, err)) => format!(
                    "rsprobe {} {}",
                    if ok { "ok" } else { "FAIL" },
                    err.chars().take(90).collect::<String>()
                ),
                None => "rsprobe n/a".into(),
            };
            eprintln!("{:55} | {ng} | {rs}", d.id);
        }
    }
}
