//! Scale-ladder deck generators (owner: H2).
//!
//! [`generate`] returns the scaling families of SUITE_SPEC section 6 as complete, self-describing decks (title line,
//! `* bench.*` metadata, one analysis card, `.end`). Everything is deterministic: the only randomness is the node
//! renumbering of `mesh_dc_shuffled`, drawn from an own xorshift64* ([`Rng`]) seeded by `seed`.
//!
//! # Scales
//!
//! The size lists below are *increments*: `Small` returns the small list, `Medium` returns small + medium, `Large`
//! returns small + medium + large (sizes de-duplicated, ascending). A size is the problem size of the family (MNA
//! unknowns for the circuit families, deck lines for `parse_stress_*`); the id of a deck is `fam_<family>_<size>`.
//! Generator memory (Rule 9, measured): `Small` < 4 MiB, `Medium` < 64 MiB (test-enforced), `Large` 94 decks
//! / 154 MiB of deck text, 188 MiB peak RSS, 1.9 s in a debug build (the 1e6-stage
//! ladder alone is 55 MiB); no search, no recursion, no unbounded loop.
//!
//! | family (id prefix)            | small          | medium                | large                     |
//! |-------------------------------|----------------|-----------------------|---------------------------|
//! | `ladder_dc` (stages)          | 200, 1000      | 1000, 10000, 100000   | 1000000                   |
//! | `mesh_dc` (m x m)             | 10, 20         | 20, 40, 60, 100, 125  | 150, 200, 250             |
//! | `mesh_dc_shuffled` (m x m)    | 10, 20         | 30, 40                | 60                        |
//! | `rc_ladder_tran` (stages)     | 100            | 1000, 10000           | 100000                    |
//! | `rc_mesh_tran` (m x m)        | 10             | 20, 60, 100           | 140                       |
//! | `lc_ladder_tran` (sections)   | 10             | 10, 100, 300          | 1000                      |
//! | `diode_chain` (diodes, sweep) | 10             | 10, 50, 500           | -                         |
//! | `diode_mesh` (m x m, sweep)   | 8              | 16, 48                | 100                       |
//! | `cmos_chain_op` / `_tran`     | 5              | 5, 50, 100, 150       | 200, 300, 400, 2000       |
//! | `mos_array_op` (g)             | 5              | 10, 20, 30            | 50, 71                    |
//! | `mos_array_tran` (g)          | 20             | 10, 30                | 50, 71                    |
//! | `mos_array_tran_fast` (g)     | 5              | -                     | -                         |
//! | `sram6t_op` / `_tran` (n x n) | 8              | 16, 24                | 32, 48                    |
//! | `star_hub` (k leaves)         | 100            | 100, 1000, 1800       | 2000                      |
//! | `parse_stress_{res,diode,subckt}` (lines) | 1000 | 1000, 10000, 100000 | 1000000               |
//! | `startup_floor`               | 1              | 1                     | 1                         |
//!
//! # Why these sizes (audit evidence, ngspice-47 vs RustSpice 7bb6d36, `audit/a5-scaling`)
//!
//! * Tier cliff: RustSpice switches solver tier at MNA dim 15 000. A natural-order mesh with m = 100 (dim 10 002)
//!   needs 0.8 s / 775 MB (ngspice SPARSE 1.4 s, KLU 84 ms); m = 120 needs 10.8 s / 1.5 GB; the first size past the
//!   cliff, m = 125 (dim 15 627), needs 0.2 s / 44 MB. m = 250 (dim 62 502) needs 0.66 s / 167 MB. The medium list
//!   straddles the cliff (100 vs 125), the large list shows the post-cliff scaling.
//! * Random numbering (`mesh_dc_shuffled`) destroys the natural band: m = 40 needs 1.4 s / 461 MB (19x slower than
//!   ngspice SPARSE... still correct), m = 60 needs 24.6 s / 4091 MB (above the 4 GiB default cap -> large only),
//!   m = 80 fails.
//! * LC ladders have a dense inductor-branch fill: n = 300 takes 0.19 s / 151 MB, n = 1000 (dim 2003) 17.6 s / 5.2 GB,
//!   n = 1100 6.9 GB, n = 1200 is OOM at 8 GiB (large only, expected to be killed by the RSS cap).
//! * Star hubs (one node with k leaves) have a sharp cliff at MNA dim ~2000: k = 1000 takes 16 ms / 5 MB and k = 1800
//!   0.02 s / 18 MiB, k = 2000 (dim 2003) does not finish (audit: > 100 s and 5.4 GB; re-measured: still growing at
//!   7.1 GiB after 46 s), k = 3000 did not finish in 361 s.
//! * 1-D ladders scale cleanly (~1.7 KB per unknown, 1e6 stages: 5.5 s, 1.7 GiB; ngspice 24-42 s).
//! * CMOS inverter chains: RustSpice solves `.op` up to n = 300 (2.7 ms) but fails at n >= 400 (150 iterations, residual
//!   5.1e-5 A) and fails transients at n >= 200 (initial operating point, NaN residual after 2 iterations); ngspice
//!   solves n = 2000 `.op` in 0.18 s. Re-run on 7bb6d36: `.op` n <= 300 pass, n = 400 and 2000 are
//!   `known_gap_confirmed`; `.tran` n <= 150 pass, n >= 200 are `known_gap_confirmed`.
//! * MOS arrays hit RustSpice's fixed absolute KCL residual (1e-12 A, not loosenable through `.options`), but only at
//!   some sizes (calibration runs with the probe): `.op` fails at g = 10 and passes at g = 5, 20, 30, 50, 71 (g = 71:
//!   4.1 s vs ngspice-KLU 0.6 s); `.tran` (300 ps edge, 50 fF) fails at g = 10 (same stall in the initial operating
//!   point), passes at g = 20, 30, 50 (g = 50: 28.7 s vs 1.9 s) and does not finish within 90 s at g = 71 (565 MiB, 420 s
//!   wall and 4100 s cpu without a result). Only these sizes carry a `known_gap`. The 100 ps / 5 fF variant
//!   (`mos_array_tran_fast`) fails the transient Newton at the switching edge at some sizes for a different reason.
//! * 6T SRAM arrays: `.op` n = 48 solves in 2-6 s, the 100-step `.tran` n = 48 needs 166 s wall / 1230 s cpu on the faer
//!   tier against 4.2 s for ngspice-KLU (`faer-nonlinear-slow`, `known_gap_confirmed` as a timeout).
//!
//! # RSS estimates (Rule 9)
//!
//! `est_rss_mib` = 1.25 x the MEASURED peak resident memory of the larger child (RustSpice or ngspice, either solver;
//! the engine's cap applies to every child) + 8 MiB, from the table `PEAK_MIB` (method and the three extrapolated
//! entries documented there). Cases above `--max-rss-mib` become `skipped_budget` in the engine, so the tables are part
//! of the contract with the report; only lc_ladder 1000, mesh_dc_shuffled 60 and star_hub 2000 exceed the default
//! 4096 MiB cap and are therefore skipped (the engine runs cases one at a time).
//!
//! # Timing/reference policy
//!
//! * `bench.ng.solver: klu` is used where the audit measured ngspice SPARSE above 10 s for that size (repeating such
//!   a run 8 times would dominate the suite; SPARSE is already represented by the smaller sizes of the same family).
//! * Transients use `tstop >= 50 tstep` so that ngspice's default `tmax = tstep`; large transients use
//!   `bench.ref: none` (a `tmax = tstep/20` reference run would cost 20x the already long ngspice run); small ones keep
//!   the engine's default converged reference. `bench.tol.norm` values were calibrated with the audit probe against a
//!   converged ngspice reference (see the per-family comments).
//!
//! `Large` is *not* run in CI; its memory and run-time are protected by the engine's `--max-rss-mib` and `--case-timeout`.

use super::types::{GeneratedDeck, Scale};
use std::fmt::Write as _;

// ------------------------------------------------------------------------------------------------ deterministic RNG

/// xorshift64* generator (no external crate, no clock).
#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    /// Seeded generator; the seed is scrambled with splitmix64 so that nearby seeds give unrelated streams.
    pub fn new(seed: u64) -> Rng {
        let mut z = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        Rng(if z == 0 { 0x2545_F491_4F6C_DD1D } else { z })
    }
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    /// Uniform in `[0, 1)`.
    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
    /// Uniform integer in `[0, n)` (`n > 0`).
    pub fn below(&mut self, n: usize) -> usize {
        ((self.next_u64() >> 11) % n.max(1) as u64) as usize
    }
    /// Uniform in `[lo, hi]`.
    pub fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.next_f64()
    }
    /// Log-uniform in `[lo, hi]` (both > 0).
    pub fn log_range(&mut self, lo: f64, hi: f64) -> f64 {
        (lo.ln() + (hi.ln() - lo.ln()) * self.next_f64()).exp()
    }
    /// Fisher-Yates shuffle.
    pub fn shuffle<T>(&mut self, v: &mut [T]) {
        for i in (1..v.len()).rev() {
            let j = self.below(i + 1);
            v.swap(i, j);
        }
    }
}

// ------------------------------------------------------------------------------------------------ formatting helpers

/// SPICE-parsable number: integers verbatim, everything else in `{:e}` notation (full round-trip precision).
pub fn num(x: f64) -> String {
    if x == x.trunc() && x.abs() < 1e15 {
        format!("{}", x as i64)
    } else {
        format!("{x:e}")
    }
}

/// Metadata of one deck (rendered as `* bench.*` comment lines).
#[derive(Clone, Debug, Default)]
pub struct Meta {
    pub id: String,
    pub group: &'static str,
    pub title: String,
    pub probes: Vec<String>,
    pub tol_rel: Option<f64>,
    pub tol_abs: Option<f64>,
    pub tol_norm: Option<f64>,
    pub floor: Option<f64>,
    pub expect: Option<&'static str>,
    pub known_gap: Option<&'static str>,
    pub gap_note: Option<String>,
    pub ng_solver: Option<&'static str>,
    pub refmode: Option<String>,
    pub ref_tmax: Option<&'static str>,
    pub oracle: Option<String>,
    pub size: u64,
    pub est_rss_mib: u64,
    pub tags: Vec<String>,
    pub timing: Option<bool>,
    pub note: Option<String>,
}

impl Meta {
    /// Render the metadata block (each line is a comment for both simulators).
    pub fn render(&self) -> String {
        let mut s = String::new();
        let mut line = |k: &str, v: String| {
            let _ = writeln!(s, "* bench.{k}: {v}");
        };
        line("id", self.id.clone());
        line("group", self.group.to_string());
        line("title", self.title.clone());
        if !self.probes.is_empty() {
            line("probes", self.probes.join(", "));
        }
        if let Some(v) = self.tol_rel {
            line("tol.rel", format!("{v:e}"));
        }
        if let Some(v) = self.tol_abs {
            line("tol.abs", format!("{v:e}"));
        }
        if let Some(v) = self.tol_norm {
            line("tol.norm", format!("{v:e}"));
        }
        if let Some(v) = self.floor {
            line("floor", format!("{v:e}"));
        }
        if let Some(v) = self.expect {
            line("expect", v.to_string());
        }
        if let Some(v) = self.known_gap {
            line("known_gap", v.to_string());
        }
        if let Some(v) = &self.gap_note {
            line("gap.note", v.clone());
        }
        if let Some(v) = self.ng_solver {
            line("ng.solver", v.to_string());
        }
        if let Some(v) = &self.refmode {
            line("ref", v.clone());
        }
        if let Some(v) = self.ref_tmax {
            line("ref.tmax", v.to_string());
        }
        if let Some(v) = &self.oracle {
            line("oracle", v.clone());
        }
        line("size", self.size.to_string());
        line("est_rss_mib", self.est_rss_mib.to_string());
        line("tags", self.tags.join(", "));
        if let Some(v) = self.timing {
            line("timing", if v { "yes" } else { "no" }.to_string());
        }
        if let Some(v) = &self.note {
            line("note", v.clone());
        }
        s
    }
}

/// Assemble title + metadata + body (body must contain the analysis card and end with `.end`).
fn assemble(title: &str, m: &Meta, body: &str) -> String {
    let mut s = String::with_capacity(body.len() + 1024);
    s.push_str(title);
    s.push('\n');
    s.push_str(&m.render());
    s.push_str(body);
    s
}

fn finish(m: Meta, family: &str, title_line: &str, body: String) -> GeneratedDeck {
    let deck = assemble(title_line, &m, &body);
    GeneratedDeck {
        id: m.id.clone(),
        family: family.to_string(),
        size: m.size,
        est_rss_mib: m.est_rss_mib,
        deck,
    }
}

// ------------------------------------------------------------------------------------------------ RSS evidence

/// Measured peak resident memory (MiB) of the LARGER of the children the engine starts for the deck (RustSpice, ngspice
/// SPARSE, ngspice KLU; the engine's `--max-rss-mib` cap applies to every child), per family and generator size
/// (`n` stages, `m`/`g` mesh side, `k` leaves, `lines`). Method: `wait4` of a fresh child of a 10 MiB parent, `VmHWM`
/// polled every 2 ms from `/proc` (the maximum of both, and of RustSpice's own `--stats peak_rss_kib`), the decks exactly
/// as generated, harness protocol (`run <deck> --raw <file>`), 7bb6d36 release build, ngspice-47, `--scale large` decks,
/// each under a 3 GiB cap. The engine's own `max_rss_kib` is NOT usable for small decks: `exec` of a vfork-style child
/// inherits the parent's resident high-water mark, so every case reports at least the engine's own RSS (see the
/// open issue on `rustsim.rs`). The floor of 19 MiB is ngspice's start-up size plus the measuring parent.
///
/// Entries marked `extrapolated` could not be measured under 3 GiB: lc_ladder 1000 (log-log fit of 700/800 gives 5150
/// MiB, audit 5167), mesh_dc_shuffled 60 (fit of 50/55 gives 4370 MiB, audit 4091), star_hub 2000 (the symbolic LU
/// passes the 3 GiB cap after 42 s; 7200 MiB observed after 46 s while still growing).
#[rustfmt::skip]
const PEAK_MIB: &[(&str, &[(u64, u64)])] = &[
    ("startup_floor", &[(1, 19)]),
    ("ladder_dc", &[(200, 19), (1000, 19), (10_000, 38), (100_000, 252), (1_000_000, 2411)]),
    ("mesh_dc", &[(10, 19), (20, 19), (40, 32), (60, 114), (100, 778), (125, 63), (150, 84), (200, 142), (250, 217)]),
    ("mesh_dc_shuffled", &[(10, 19), (20, 19), (30, 91), (40, 382), (45, 774), (50, 1546), (55, 2664), (60, 4370)]), // 60 extrapolated
    ("rc_ladder_tran", &[(100, 19), (1000, 19), (10_000, 60), (100_000, 525)]),
    ("rc_mesh_tran", &[(10, 19), (20, 19), (60, 114), (100, 715), (140, 97)]),
    ("lc_ladder_tran", &[(10, 19), (100, 19), (300, 153), (400, 344), (500, 659), (600, 1138), (700, 1794), (800, 2662), (1000, 5167)]), // 1000 extrapolated
    ("diode_chain", &[(10, 19), (50, 19), (500, 19)]),
    ("diode_mesh", &[(8, 19), (16, 19), (48, 59), (100, 718)]),
    ("cmos_chain_op", &[(5, 19), (50, 19), (100, 19), (150, 19), (200, 19), (300, 19), (400, 19), (2000, 29)]),
    ("cmos_chain_tran", &[(5, 19), (50, 19), (100, 19), (150, 19), (200, 19), (300, 19), (400, 19), (2000, 30)]),
    ("mos_array_op", &[(5, 19), (10, 19), (20, 22), (30, 85), (50, 476), (71, 546)]),
    ("mos_array_tran", &[(5, 19), (10, 19), (20, 22), (30, 86), (50, 476), (71, 565)]),
    ("sram6t_op", &[(8, 19), (16, 24), (24, 32), (32, 147), (48, 580)]),
    ("sram6t_tran", &[(8, 19), (16, 23), (24, 33), (32, 154), (48, 605)]),
    ("star_hub", &[(100, 19), (1000, 19), (1800, 19), (1990, 19), (2000, 7200)]), // 2000 extrapolated
    ("parse_stress_res", &[(1000, 19), (10_000, 27), (100_000, 131), (1_000_000, 1183)]),
    ("parse_stress_diode", &[(1000, 19), (10_000, 37), (100_000, 235), (1_000_000, 2255)]),
    ("parse_stress_subckt", &[(1000, 19), (10_000, 38), (100_000, 247), (1_000_000, 2347)]),
];

/// Log-log interpolation of `table` (sorted by size) at `x`, extrapolating with the slope of the nearest segment.
fn interp(table: &[(u64, u64)], x: f64) -> f64 {
    match table {
        [] => 32.0,
        [(_, y)] => *y as f64,
        _ => {
            let seg = table
                .windows(2)
                .find(|w| x <= w[1].0 as f64)
                .unwrap_or(&table[table.len() - 2..]);
            let ((x0, y0), (x1, y1)) = (
                (seg[0].0 as f64, seg[0].1 as f64),
                (seg[1].0 as f64, seg[1].1 as f64),
            );
            let t = (x.ln() - x0.ln()) / (x1.ln() - x0.ln());
            (y0.ln() + t * (y1.ln() - y0.ln())).exp()
        }
    }
}

/// `est_rss_mib` of a generated deck: 1.25 x the measured peak (an upper bound with 25 % margin; the measurements are
/// repeatable to a few percent) + 8 MiB; sizes between table points are log-log interpolated, beyond them extrapolated.
fn est_rss(family: &str, size: u64) -> u64 {
    let table = PEAK_MIB
        .iter()
        .find(|(f, _)| *f == family)
        .map_or(&[][..], |(_, t)| *t);
    let peak = match table.iter().find(|(s, _)| *s == size) {
        Some((_, p)) => *p as f64,
        None => interp(table, size as f64),
    };
    (peak * 1.25).ceil() as u64 + 8
}

// ------------------------------------------------------------------------------------------------ shared model cards

const MOS_MODELS: &str = ".model NMOD NMOS(LEVEL=1 VTO=0.7 KP=120u LAMBDA=0.02)\n.model PMOD PMOS(LEVEL=1 VTO=-0.7 KP=60u LAMBDA=0.02)\n";
const DIODE_MODEL: &str = ".model DMOD D(IS=1e-14 N=1.0)\n";

fn probe_nodes(prefix: &str, n: usize) -> Vec<String> {
    let mut idx = vec![1, n / 4, n / 2, 3 * n / 4, n];
    idx.retain(|&i| i >= 1);
    idx.sort_unstable();
    idx.dedup();
    idx.into_iter().map(|i| format!("v({prefix}{i})")).collect()
}

fn base_meta(id: String, family: &str, title: String, size: u64, est_rss: u64) -> Meta {
    Meta {
        id,
        group: "performance",
        title,
        size,
        est_rss_mib: est_rss,
        tags: vec![format!("family:{family}")],
        expect: Some("match"),
        timing: Some(true),
        ..Meta::default()
    }
}

// ------------------------------------------------------------------------------------------------ ladder_dc

/// Series and shunt resistance of the `n`-stage ladder. The shunt is chosen so that the decay length of the line is
/// `n / 5` stages: every node stays O(1) V (the first audit decks compared 1e-34 V values); with the open far end the
/// smallest node is `1 / cosh(5) = 0.0135` V.
pub fn ladder_params(n: u64) -> (f64, f64) {
    let rsh = ((n as f64 / 5.0).powi(2)).max(100.0).round();
    (1.0, rsh)
}

/// Exact node voltages `v[0..=n]` of the ladder `V1 n0 0 1; Rs n_{k-1} n_k rs; Rsh n_k 0 rsh` (open at the far end),
/// by the O(n) impedance recursion `Z_n = rsh`, `Z_k = rsh || (rs + Z_{k+1})`, `v_k = v_{k-1} Z_k / (rs + Z_k)`.
pub fn ladder_exact(n: usize, rs: f64, rsh: f64) -> Vec<f64> {
    let mut z = vec![0.0; n + 1];
    z[n] = rsh;
    for k in (1..n).rev() {
        let s = rs + z[k + 1];
        z[k] = rsh * s / (rsh + s);
    }
    let mut v = vec![0.0; n + 1];
    v[0] = 1.0;
    for k in 1..=n {
        v[k] = v[k - 1] * z[k] / (rs + z[k]);
    }
    v
}

fn gen_ladder_dc(n: u64, _seed: u64) -> GeneratedDeck {
    let (rs, rsh) = ladder_params(n);
    let dim = n + 2;
    let mut m = base_meta(
        format!("fam_ladder_dc_{n}"),
        "ladder_dc",
        format!("1-D resistor ladder, {n} stages, .op"),
        dim,
        est_rss("ladder_dc", n),
    );
    m.probes = probe_nodes("n", n as usize);
    m.probes.push("i(v1)".into());
    m.tol_rel = Some(1e-6);
    m.tol_abs = Some(1e-9);
    // dense LU of the deck itself where it fits, the closed form of the intended circuit beyond (oracle.rs `ladder`)
    m.oracle = Some(if dim <= super::oracle::DC_DENSE_LIMIT as u64 {
        "dc_linear".to_string()
    } else {
        format!("ladder(n={n},rs={},rsh={})", num(rs), num(rsh))
    });
    if n >= 1_000_000 {
        m.ng_solver = Some("klu");
        // Both direct solvers lose ~1e-6 relative accuracy on this chain (condition number ~ rsh/rs = 4e10): against the
        // exact closed form tight ngspice is off by 3.3e-7 V (1.2e-6 relative) at n250000 and RustSpice by 1.6e-8; at the
        // far end RustSpice -1.5e-8 and ngspice +1.1e-8. The reference's own error sets the tolerance (SUITE_SPEC 13.4 rule 2).
        m.tol_rel = Some(1e-5);
    }
    m.tags.extend(["linear".into(), "scaling".into()]);
    m.note = Some(format!(
        "series {} ohm, shunt {} ohm (decay length n/5 stages, all nodes O(1) V); exact closed form in oracle.rs (ladder) and recursion in families.rs; RustSpice ~1.7 KB/unknown{}",
        num(rs),
        num(rsh),
        if n >= 1_000_000 {
            "; ngspice SPARSE needs ~42 s here, KLU only; tol.rel 1e-5 = the direct solvers' own accuracy on this chain (tight ngspice 1.2e-6 relative off the exact solution at n250000, RustSpice 5e-8)"
        } else {
            ""
        }
    ));
    let mut b = String::with_capacity(n as usize * 40 + 64);
    let _ = writeln!(b, "V1 n0 0 DC 1");
    for k in 1..=n {
        let _ = writeln!(b, "Rs{k} n{} n{k} {}", k - 1, num(rs));
        let _ = writeln!(b, "Rp{k} n{k} 0 {}", num(rsh));
    }
    b.push_str(".op\n.end\n");
    finish(m, "ladder_dc", &format!("ladder_dc n={n}"), b)
}

// ------------------------------------------------------------------------------------------------ mesh_dc

fn mesh_elements(m: usize) -> Vec<String> {
    let mut el = vec!["V1 vdd 0 DC 1".to_string()];
    for i in 0..m {
        for j in 0..m {
            if j + 1 < m {
                el.push(format!("Rh{i}_{j} n{i}_{j} n{i}_{} 1", j + 1));
            }
            if i + 1 < m {
                el.push(format!("Rv{i}_{j} n{i}_{j} n{}_{j} 1", i + 1));
            }
        }
    }
    for (c, (i, j)) in [(0, 0), (0, m - 1), (m - 1, 0), (m - 1, m - 1)]
        .iter()
        .enumerate()
    {
        el.push(format!("Rpad{c} vdd n{i}_{j} 0.01"));
    }
    let mut k = 0;
    for a in 0..3 {
        for b in 0..3 {
            let i = ((m as f64 * (a as f64 + 0.5) / 3.0) as usize).min(m - 1);
            let j = ((m as f64 * (b as f64 + 0.5) / 3.0) as usize).min(m - 1);
            // 10 mA per tap: IR droop of tens of mV, so the 1e-6 relative tolerance resolves it
            el.push(format!("I{k} n{i}_{j} 0 DC 10m"));
            k += 1;
        }
    }
    el
}

fn mesh_probes(m: usize) -> Vec<String> {
    let c = m / 2;
    vec![
        "v(n0_0)".to_string(),
        format!("v(n{c}_{c})"),
        format!("v(n{}_{})", m - 1, m / 3),
        format!("v(n{}_{})", m - 1, m - 1),
        "i(v1)".to_string(),
    ]
}

fn mesh_ng_solver(m: u64) -> Option<&'static str> {
    // audit: ngspice SPARSE 27 s at m=110, 33-60 s at m=125..150, > 60 s beyond
    if m >= 110 {
        Some("klu")
    } else {
        None
    }
}

fn gen_mesh_dc(m: u64, _seed: u64) -> GeneratedDeck {
    let dim = m * m + 2;
    let mut meta = base_meta(
        format!("fam_mesh_dc_{m}"),
        "mesh_dc",
        format!("2-D resistor mesh {m}x{m} (natural numbering), .op"),
        dim,
        est_rss("mesh_dc", m),
    );
    meta.probes = mesh_probes(m as usize);
    meta.tol_rel = Some(1e-6);
    meta.tol_abs = Some(1e-9);
    if dim <= super::oracle::DC_DENSE_LIMIT as u64 {
        meta.oracle = Some("dc_linear".into());
    }
    meta.ng_solver = mesh_ng_solver(m);
    meta.tags
        .extend(["linear".into(), "scaling".into(), "tier_cliff".into()]);
    meta.note = Some(format!(
        "1 ohm mesh, 4 corner pads (0.01 ohm) to a 1 V rail, 9 taps of 10 mA; RustSpice switches solver tier at dim 15000 (m=122): m=100 0.8 s/775 MB, m=120 10.8 s/1.5 GB, m=125 0.2 s/44 MB (audit a5); ngspice SPARSE 1.4 s at m=100, KLU 84 ms{}",
        if m >= 110 { "; ngspice SPARSE needs 27 s or more here, KLU only" } else { "" }
    ));
    let mut b = String::new();
    for l in mesh_elements(m as usize) {
        b.push_str(&l);
        b.push('\n');
    }
    b.push_str(".op\n.end\n");
    finish(meta, "mesh_dc", &format!("mesh_dc m={m}"), b)
}

fn gen_mesh_dc_shuffled(m: u64, seed: u64) -> GeneratedDeck {
    let dim = m * m + 2;
    let mut meta = base_meta(
        format!("fam_mesh_dc_shuffled_{m}"),
        "mesh_dc_shuffled",
        format!("2-D resistor mesh {m}x{m}, random node numbering, .op"),
        dim,
        est_rss("mesh_dc_shuffled", m),
    );
    meta.probes = mesh_probes(m as usize);
    meta.tol_rel = Some(1e-6);
    meta.tol_abs = Some(1e-9);
    if dim <= super::oracle::DC_DENSE_LIMIT as u64 {
        meta.oracle = Some("dc_linear".into());
    }
    meta.tags
        .extend(["linear".into(), "scaling".into(), "ordering".into()]);
    let mut note = String::from(
        "same circuit as mesh_dc with the element lines randomly permuted (RustSpice numbers nodes by first appearance): m=40 takes 1.4 s/461 MB (19x ngspice SPARSE, performance gap only), m=80 fails",
    );
    if m == 60 {
        note.push_str("; est_rss above the default --max-rss-mib: large scale only");
    }
    meta.note = Some(note);
    let mut el = mesh_elements(m as usize);
    let mut rng = Rng::new(seed ^ 0x5348_5546_u64 ^ (m << 20));
    rng.shuffle(&mut el);
    let mut b = String::new();
    for l in el {
        b.push_str(&l);
        b.push('\n');
    }
    b.push_str(".op\n.end\n");
    finish(
        meta,
        "mesh_dc_shuffled",
        &format!("mesh_dc_shuffled m={m}"),
        b,
    )
}

// ------------------------------------------------------------------------------------------------ transients (RC/LC)

fn tran_meta_common(meta: &mut Meta, tol_norm: f64, floor: f64, ref_none: bool) {
    meta.tol_norm = Some(tol_norm);
    meta.floor = Some(floor);
    if ref_none {
        meta.refmode = Some("none".to_string());
    }
}

fn gen_rc_ladder_tran(n: u64, _seed: u64) -> GeneratedDeck {
    let dim = n + 2;
    let mut meta = base_meta(
        format!("fam_rc_ladder_tran_{n}"),
        "rc_ladder_tran",
        format!("RC ladder {n} stages, PULSE (200 ps edges), .tran 10p 5n"),
        dim,
        est_rss("rc_ladder_tran", n),
    );
    let mut p = vec!["v(n1)".to_string()];
    for k in [10u64, 100, 300] {
        if k <= n {
            p.push(format!("v(n{k})"));
        }
    }
    meta.probes = p;
    // calibrated with the audit probe vs converged ngspice on n=100: worst 0.37 % of swing with 200 ps edges (20 steps
    // per edge; the audit's 10 ps edges = 1 step gave 7 %, which measures RustSpice's fixed-step PULSE handling, not scaling)
    tran_meta_common(&mut meta, 1e-2, 1e-3, n >= 10_000);
    if n <= 100 {
        meta.oracle = Some("tran_lti".into());
    }
    meta.tags
        .extend(["linear".into(), "tran".into(), "scaling".into()]);
    meta.note = Some(format!(
        "R=5 ohm, C=10 fF per stage, 500 steps, 200 ps edges (20 steps per edge); the front reaches ~sqrt(5ns/50fs)=316 stages, farther nodes are vacuous and excluded from probes{}",
        if n >= 100_000 { "; ngspice needs ~80 s here (audit: raise --case-timeout); RustSpice 26 s/513 MB" } else { "" }
    ));
    let mut b = String::with_capacity(n as usize * 40 + 128);
    let _ = writeln!(b, "V1 n0 0 PULSE(0 3.3 0 200p 200p 2n 5n)");
    for k in 1..=n {
        let _ = writeln!(b, "R{k} n{} n{k} 5", k - 1);
        let _ = writeln!(b, "C{k} n{k} 0 10f");
    }
    b.push_str(".tran 10p 5n\n.end\n");
    finish(meta, "rc_ladder_tran", &format!("rc_ladder_tran n={n}"), b)
}

fn gen_rc_mesh_tran(m: u64, _seed: u64) -> GeneratedDeck {
    let dim = m * m + 2;
    let mut meta = base_meta(
        format!("fam_rc_mesh_tran_{m}"),
        "rc_mesh_tran",
        format!("2-D RC mesh {m}x{m}, PULSE (200 ps edges), .tran 10p 2n"),
        dim,
        est_rss("rc_mesh_tran", m),
    );
    let c = m / 2;
    meta.probes = vec![
        "v(n0_0)".into(),
        format!("v(n{c}_{c})"),
        format!("v(n{}_{})", m - 1, m - 1),
    ];
    // calibrated on m=10: worst 0.34 % of swing with 200 ps edges
    tran_meta_common(&mut meta, 1e-2, 1e-3, m >= 60);
    if m <= 10 {
        meta.oracle = Some("tran_lti".into());
    }
    if m >= 100 {
        meta.ng_solver = Some("klu");
    }
    meta.tags.extend([
        "linear".into(),
        "tran".into(),
        "scaling".into(),
        "tier_cliff".into(),
    ]);
    meta.note = Some(format!(
        "R=5 ohm, C=10 fF per node, 50 ohm source resistance, 200 steps, 200 ps edges; audit: RustSpice m=100 1.3 s/716 MB vs m=140 0.3 s/92 MB (tier cliff at dim 15000); ngspice SPARSE m=100 28.8 s, KLU 4.3 s{}",
        if m >= 100 { "; KLU only" } else { "" }
    ));
    let mut b = String::new();
    b.push_str("V1 vin 0 PULSE(0 1 0 200p 200p 2n 5n)\nRsrc vin n0_0 50\n");
    for i in 0..m {
        for j in 0..m {
            let _ = writeln!(b, "C{i}_{j} n{i}_{j} 0 10f");
            if j + 1 < m {
                let _ = writeln!(b, "Rh{i}_{j} n{i}_{j} n{i}_{} 5", j + 1);
            }
            if i + 1 < m {
                let _ = writeln!(b, "Rv{i}_{j} n{i}_{j} n{}_{j} 5", i + 1);
            }
        }
    }
    b.push_str(".tran 10p 2n\n.end\n");
    finish(meta, "rc_mesh_tran", &format!("rc_mesh_tran m={m}"), b)
}

fn gen_lc_ladder_tran(n: u64, _seed: u64) -> GeneratedDeck {
    let dim = 2 * n + 3;
    let mut meta = base_meta(
        format!("fam_lc_ladder_tran_{n}"),
        "lc_ladder_tran",
        format!("LC ladder {n} sections (Z0 = 31.6 ohm), .tran 50p 20n"),
        dim,
        est_rss("lc_ladder_tran", n),
    );
    let mut p = vec!["v(a1)".to_string()];
    for k in [5u64, 20, 60] {
        if k <= n {
            p.push(format!("v(a{k})"));
        }
    }
    meta.probes = p;
    // calibrated on n=10: 0.3 % of swing at 50 ps / 2 ns edges (100 ps steps gave 1.3 %, 20 ps 0.05 %): the undamped
    // ladder rings, so the error is trapezoid dispersion, which the step size controls
    tran_meta_common(&mut meta, 1e-2, 1e-3, n >= 1000);
    if n <= 10 {
        meta.oracle = Some("tran_lti".into());
    }
    meta.tags.extend([
        "linear".into(),
        "tran".into(),
        "scaling".into(),
        "ringing".into(),
    ]);
    let mut note = String::from(
        "L=10 nH, C=10 pF per section, source and load 31.6 ohm, 400 steps; one section delays 0.32 ns, so the 20 ns window reaches ~63 sections; audit: RustSpice n=300 0.19 s/151 MB (symbolic LU O(n^3) on the inductor branches), ngspice 16 ms",
    );
    if n == 1000 {
        note.push_str("; est_rss above the default --max-rss-mib: large scale only");
    }
    meta.note = Some(note);
    let mut b = String::new();
    b.push_str("V1 in 0 PULSE(0 1 0 2n 2n 6n 20n)\nRs in a0 31.6\n");
    for k in 1..=n {
        let _ = writeln!(b, "L{k} a{} a{k} 10n", k - 1);
        let _ = writeln!(b, "C{k} a{k} 0 10p");
    }
    let _ = writeln!(b, "Rload a{n} 0 31.6");
    b.push_str(".tran 50p 20n\n.end\n");
    finish(meta, "lc_ladder_tran", &format!("lc_ladder_tran n={n}"), b)
}

// ------------------------------------------------------------------------------------------------ diodes

fn gen_diode_chain(n: u64, _seed: u64) -> GeneratedDeck {
    let dim = n + 2;
    let vmax = 0.65 * n as f64 + 1.0;
    let mut meta = base_meta(
        format!("fam_diode_chain_{n}"),
        "diode_chain",
        format!("{n} series diodes behind 1k, .dc sweep 101 points"),
        dim,
        est_rss("diode_chain", n),
    );
    meta.probes = vec!["v(d0)".into(), format!("v(d{})", n / 2), "i(v1)".into()];
    meta.tol_rel = Some(1e-5);
    meta.tol_abs = Some(1e-8);
    if dim <= 60 {
        meta.oracle = Some("dc_nonlinear".into());
    }
    meta.tags
        .extend(["nonlinear".into(), "sweep".into(), "scaling".into()]);
    meta.note = Some("IS=1e-14 N=1, sweep 0..0.65n+1 V in 101 steps; RustSpice 0.7-5.5 ms for n<=500 (audit); the dense oracle covers n<=50 only".into());
    let mut b = String::new();
    b.push_str(DIODE_MODEL);
    let _ = writeln!(b, "V1 in 0 DC {}", num(vmax));
    b.push_str("R1 in d0 1000\n");
    for k in 1..=n {
        let nxt = if k == n {
            "0".to_string()
        } else {
            format!("d{k}")
        };
        let _ = writeln!(b, "D{k} d{} {nxt} DMOD", k - 1);
    }
    let _ = writeln!(b, ".dc V1 0 {} {}", num(vmax), num(vmax / 100.0));
    b.push_str(".end\n");
    finish(meta, "diode_chain", &format!("diode_chain n={n}"), b)
}

fn gen_diode_mesh(m: u64, _seed: u64) -> GeneratedDeck {
    let dim = m * m + 2;
    let mut meta = base_meta(
        format!("fam_diode_mesh_{m}"),
        "diode_mesh",
        format!("diode-resistor mesh {m}x{m}, .dc 31 points"),
        dim,
        est_rss("diode_mesh", m),
    );
    let c = m / 2;
    meta.probes = vec![
        "v(n0_0)".into(),
        format!("v(n{c}_{c})"),
        format!("v(n{}_{})", m - 1, m - 1),
        "i(v1)".into(),
    ];
    meta.tol_rel = Some(1e-5);
    meta.tol_abs = Some(1e-8);
    if dim <= 100 {
        meta.oracle = Some("dc_nonlinear".into());
    }
    meta.tags
        .extend(["nonlinear".into(), "sweep".into(), "scaling".into()]);
    meta.note = Some("one diode per node to ground, 100 ohm mesh, 100 ohm source resistance; audit: RustSpice faer tier is slower than ngspice-KLU on nonlinear meshes (faer-nonlinear-slow, 3-70x, performance only): m=48 0.30 s vs KLU 0.09 s, m=100 1.6 s/716 MB vs KLU 0.8 s".into());
    let mut b = String::new();
    b.push_str(DIODE_MODEL);
    b.push_str("V1 in 0 DC 3\nRin in n0_0 100\n");
    for i in 0..m {
        for j in 0..m {
            let _ = writeln!(b, "D{i}_{j} n{i}_{j} 0 DMOD");
            if j + 1 < m {
                let _ = writeln!(b, "Rh{i}_{j} n{i}_{j} n{i}_{} 100", j + 1);
            }
            if i + 1 < m {
                let _ = writeln!(b, "Rv{i}_{j} n{i}_{j} n{}_{j} 100", i + 1);
            }
        }
    }
    b.push_str(".dc V1 0 3 0.1\n.end\n");
    finish(meta, "diode_mesh", &format!("diode_mesh m={m}"), b)
}

// ------------------------------------------------------------------------------------------------ CMOS / MOS

fn cmos_chain_body(n: u64, tran: bool) -> String {
    let mut b = String::new();
    b.push_str(MOS_MODELS);
    b.push_str("VDD vdd 0 DC 3.3\nVIN in1 0 PULSE(0.0 3.3 0.2n 0.1n 0.1n 2n 4n)\n");
    for k in 1..=n {
        let inp = if k == 1 {
            "in1".to_string()
        } else {
            format!("n{}", k - 1)
        };
        let _ = writeln!(b, "MP{k} n{k} {inp} vdd vdd PMOD W=20u L=1u");
        let _ = writeln!(b, "MN{k} n{k} {inp} 0 0 NMOD W=10u L=1u");
        let _ = writeln!(b, "CL{k} n{k} 0 20f");
    }
    b.push_str(if tran {
        ".tran 5p 2n\n.end\n"
    } else {
        ".op\n.end\n"
    });
    b
}

fn gen_cmos_chain_op(n: u64, _seed: u64) -> GeneratedDeck {
    let mut meta = base_meta(
        format!("fam_cmos_chain_op_{n}"),
        "cmos_chain_op",
        format!("{n}-stage CMOS inverter chain, .op"),
        n + 4,
        est_rss("cmos_chain_op", n),
    );
    // supply current is leakage-level (1e-11 A, vacuous) with the input low: not probed
    meta.probes = vec!["v(n1)".into(), "v(n2)".into(), format!("v(n{})", n)];
    meta.tol_rel = Some(1e-6);
    meta.tol_abs = Some(1e-8);
    meta.tags
        .extend(["nonlinear".into(), "mos".into(), "scaling".into()]);
    let note = String::from("MOS level 1 (VTO 0.7, KP 120u/60u, LAMBDA 0.02), W 10u/20u, L 1u; input at 0 so the chain alternates rails; audit: RustSpice n<=300 2.7 ms vs ngspice 18 ms");
    meta.note = Some(note);
    finish(
        meta,
        "cmos_chain_op",
        &format!("cmos_chain_op n={n}"),
        cmos_chain_body(n, false),
    )
}

fn gen_cmos_chain_tran(n: u64, _seed: u64) -> GeneratedDeck {
    let mut meta = base_meta(
        format!("fam_cmos_chain_tran_{n}"),
        "cmos_chain_tran",
        format!("{n}-stage CMOS inverter chain, .tran 5p 2n"),
        n + 4,
        est_rss("cmos_chain_tran", n),
    );
    meta.probes = vec!["v(n1)".into(), "v(n2)".into(), "v(n3)".into()];
    // calibrated with the audit probe on n=5: worst 1.2 % of swing (fixed-step trapezoid on the 100 ps edges)
    tran_meta_common(&mut meta, 3e-2, 1e-2, n >= 400);
    meta.tags.extend([
        "nonlinear".into(),
        "mos".into(),
        "tran".into(),
        "scaling".into(),
        "timing_edge".into(),
    ]);
    let note = String::from("input edge at 0.2 ns propagates ~50 ps per stage, so only the first ~40 stages move within 2 ns (later nodes are vacuous and not probed); 400 steps");
    meta.note = Some(note);
    finish(
        meta,
        "cmos_chain_tran",
        &format!("cmos_chain_tran n={n}"),
        cmos_chain_body(n, true),
    )
}

/// `.op` deck, or one of the two `.tran` decks: `fast` = 100 ps input edge and 5 fF outputs (the edge RustSpice's
/// transient Newton cannot follow at some sizes), otherwise 300 ps edge and 50 fF outputs, which both simulators resolve
/// at the 20 ps step (calibration against ngspice with default tolerances and `tmax = 2 ps`: RustSpice 0.5 % of swing,
/// ngspice as-is 0.6 %; with 5 fF outputs the same step leaves BOTH at 10-12 %).
fn mos_array_body(g: u64, tran: bool, fast: bool) -> String {
    let (edge, cl) = if fast {
        ("0.1n", "5f")
    } else {
        ("0.3n", "50f")
    };
    let mut b = String::new();
    b.push_str(MOS_MODELS);
    let _ = writeln!(
        b,
        "VDD vdd 0 DC 3.3\nVIN in 0 PULSE(0.0 3.3 0.2n {edge} {edge} 2n 4n)\nRpad vdd v0_0 0.1"
    );
    for i in 0..g {
        for j in 0..g {
            if j + 1 < g {
                let _ = writeln!(b, "RVh{i}_{j} v{i}_{j} v{i}_{} 1", j + 1);
            }
            if i + 1 < g {
                let _ = writeln!(b, "RVv{i}_{j} v{i}_{j} v{}_{j} 1", i + 1);
            }
            let _ = writeln!(b, "Rg{i}_{j} in g{i}_{j} 100");
            let _ = writeln!(b, "Cg{i}_{j} g{i}_{j} 0 2f");
            let _ = writeln!(
                b,
                "MP{i}_{j} o{i}_{j} g{i}_{j} v{i}_{j} v{i}_{j} PMOD W=20u L=1u"
            );
            let _ = writeln!(b, "MN{i}_{j} o{i}_{j} g{i}_{j} 0 0 NMOD W=10u L=1u");
            let _ = writeln!(b, "CL{i}_{j} o{i}_{j} 0 {cl}");
        }
    }
    b.push_str(if tran {
        ".tran 20p 1n\n.end\n"
    } else {
        ".op\n.end\n"
    });
    b
}

/// Known gaps of the MOS arrays, each re-measured with the shipped binary (7bb6d36) and a floating reference:
///
/// * `.op` and `.tran` at g = 10: the Newton iteration of the operating point stalls at an absolute KCL residual of
///   3.1e-11 A (150 iterations) against the fixed 1e-12 A limit. The stall residual is identical for device widths
///   scaled 0.1x ... 100x (so it is not a device-current scale effect) and appears for g = 9, 10, 12, 15 but not for
///   g = 5, 8, 11; shrinking the rail resistances (1 mohm links) moves it to 6e-10 A, i.e. it follows the conductance
///   scale of the rail mesh like a rounding floor. The correct answer is trivial and independent of any simulator: every
///   NMOS is off (inputs at 0 V) so no DC current flows and every rail/output node equals VDD (tight ngspice: 3.3 V
///   within 1.6e-9, gates exactly 0 V). `.options` cannot loosen the limit.
/// * `.tran` at g = 71: no result within 90 s on the faer tier (g = 50 about 29 s; ngspice-KLU 1.9 s and 4.6 s); counts
///   as a timeout at the default case timeout (performance gap).
///
fn mos_array_gap(meta: &mut Meta, g: u64, tran: bool) {
    let (slug, note) = match (tran, g) {
        (true, 71) => ("faer-nonlinear-slow", "RustSpice gives no result within 90 s (g=50 takes ~29 s, ngspice-KLU 1.9 s at g=50 and 4.6 s at g=71; SPARSE 109 s): the faer tier is far slower than ngspice-KLU on nonlinear systems; counts as a timeout at the default case timeout"),
        _ => return,
    };
    meta.known_gap = Some(slug);
    meta.gap_note = Some(note.into());
}

fn gen_mos_array_op(g: u64, _seed: u64) -> GeneratedDeck {
    let dim = 3 * g * g + 4;
    let mut meta = base_meta(
        format!("fam_mos_array_op_{g}"),
        "mos_array_op",
        format!(
            "{g}x{g} array of CMOS inverters ({} MOS) on a resistive rail, .op",
            2 * g * g
        ),
        dim,
        est_rss("mos_array_op", g),
    );
    let c = g / 2;
    meta.probes = vec![
        "v(o0_0)".into(),
        format!("v(o{c}_{c})"),
        format!("v(o{}_{})", g - 1, g - 1),
        format!("v(v{}_{})", g - 1, g - 1),
    ];
    meta.tol_rel = Some(1e-6);
    meta.tol_abs = Some(1e-8);
    if g >= 50 {
        meta.ng_solver = Some("klu");
    }
    meta.tags
        .extend(["nonlinear".into(), "mos".into(), "scaling".into()]);
    mos_array_gap(&mut meta, g, false);
    meta.note = Some("VDD distributed through a 1 ohm mesh, one 100 ohm gate hub, inputs at 0 V (every rail and output node sits at VDD); audit: ngspice SPARSE g=71 14 s, KLU 2.4 s, g=100 87 s/6.3 s (g >= 50 KLU only)".into());
    finish(
        meta,
        "mos_array_op",
        &format!("mos_array_op g={g}"),
        mos_array_body(g, false, false),
    )
}

fn gen_mos_array_tran(g: u64, _seed: u64) -> GeneratedDeck {
    gen_mos_array_tran_variant(g, false)
}

fn gen_mos_array_tran_fast(g: u64, _seed: u64) -> GeneratedDeck {
    gen_mos_array_tran_variant(g, true)
}

fn gen_mos_array_tran_variant(g: u64, fast: bool) -> GeneratedDeck {
    let dim = 3 * g * g + 4;
    let fam = if fast {
        "mos_array_tran_fast"
    } else {
        "mos_array_tran"
    };
    let mut meta = base_meta(
        format!("fam_{fam}_{g}"),
        fam,
        if fast {
            format!("{g}x{g} array of CMOS inverters, 100 ps input edge, .tran 20p 1n")
        } else {
            format!("{g}x{g} array of CMOS inverters, .tran 20p 1n")
        },
        dim,
        est_rss("mos_array_tran", g),
    );
    let c = g / 2;
    meta.probes = vec![
        "v(g0_0)".into(),
        format!("v(o{c}_{c})"),
        format!("v(o{}_{})", g - 1, g - 1),
    ];
    meta.tol_norm = Some(2e-2);
    meta.floor = Some(1e-2);
    if fast {
        // A converged reference, not the as-is run: at the 20 ps step with 5 fF outputs ngspice's own as-is
        // answer is 11 % of swing off the converged waveform, so the "ngspice solves it" evidence of the known gap
        // (and the case itself, once RustSpice follows the edge: a PASS against an 11 % wrong reference would be
        // meaningless, a FAIL against it a false alarm) needs a reference that is itself converged. The default
        // converged reference (reltol 1e-6, tmax tstep/20 = 1 ps) does not finish (120 s, 345 MB of rawfile at g = 5);
        // reltol 1e-3 with `tmax` 2 ps runs in 12 ms and is within 0.6 % of swing of the reltol 1e-6 / tmax 2 ps run
        // (measured with ngspice-47; the as-is run is 11 % off), far inside `tol_norm` = 2 %.
        meta.refmode = Some("reltol=1e-3 abstol=1e-12 vntol=1e-6".to_string());
        meta.ref_tmax = Some("2p");
        // Formerly `fixed-step-accuracy` (0.142 of swing at the fixed 20 ps step on the 20 ps output edges); the
        // adaptive, breakpoint-aware integrator follows the edges (5.5e-3 of swing at g = 5).
    } else if g <= 30 {
        // default ngspice tolerances with a small tmax: the tight reference (reltol 1e-6) does not finish within 100 s on
        // these decks (it is also needless: tmax 5 ps and 2 ps agree to 0.06 percent of swing at g = 20)
        meta.refmode = Some("reltol=1e-3 abstol=1e-12 vntol=1e-6".to_string());
        meta.ref_tmax = Some("2p");
    } else {
        // tmax = 2 ps costs 10x the already long ngspice run; both simulators carry ~0.5 % error at the 20 ps step, so
        // the as-is comparison stays far below the 2 % tolerance
        meta.refmode = Some("none".to_string());
    }
    if g >= 50 {
        meta.ng_solver = Some("klu");
    }
    meta.tags.extend([
        "nonlinear".into(),
        "mos".into(),
        "tran".into(),
        "scaling".into(),
        "timing_edge".into(),
    ]);
    if !fast {
        mos_array_gap(&mut meta, g, true);
    }
    meta.note = Some(if fast {
        "5 fF outputs: the 20 ps step is coarse for the 20 ps output edges (as-is ngspice is 11 % of swing off the converged waveform), so the comparison runs against a converged ngspice reference (reltol 1e-3, tmax 2 ps) - the case documents the transient Newton failure; 50 steps".into()
    } else {
        "300 ps edge, 50 fF outputs, 50 steps; both simulators are ~0.5 % off the converged waveform; audit: ngspice SPARSE g=50 25 s, g=71 109 s, KLU 5.9 s / 12.8 s".into()
    });
    finish(
        meta,
        fam,
        &format!("{fam} g={g}"),
        mos_array_body(g, true, fast),
    )
}

// ------------------------------------------------------------------------------------------------ SRAM

fn sram_body(n: u64, tran: bool) -> String {
    let (rows, cols) = (n, n);
    let mut b = String::new();
    b.push_str(MOS_MODELS);
    b.push_str("VDD vdd 0 DC 3.3\n");
    for r in 0..rows {
        if r == 0 {
            let _ = writeln!(b, "VWL{r} wld{r} 0 PULSE(0 3.3 0.2n 0.1n 0.1n 1n 4n)");
        } else {
            let _ = writeln!(b, "VWL{r} wld{r} 0 DC 0");
        }
        let _ = writeln!(b, "Rwd{r} wld{r} wl{r}_0 10");
    }
    for c in 0..cols {
        let _ = writeln!(b, "Rpc{c} vdd bl{c}_0 5k");
        let _ = writeln!(b, "Rpcb{c} vdd blb{c}_0 5k");
    }
    for r in 0..rows {
        for c in 0..cols {
            let (q, qb) = (format!("q{r}_{c}"), format!("qb{r}_{c}"));
            let blr = format!("bl{c}_{r}");
            let blbr = format!("blb{c}_{r}");
            let wlc = format!("wl{r}_{c}");
            let _ = writeln!(b, "MPL{r}_{c} {q} {qb} vdd vdd PMOD W=2u L=1u");
            let _ = writeln!(b, "MNL{r}_{c} {q} {qb} 0 0 NMOD W=4u L=1u");
            let _ = writeln!(b, "MPR{r}_{c} {qb} {q} vdd vdd PMOD W=2u L=1u");
            let _ = writeln!(b, "MNR{r}_{c} {qb} {q} 0 0 NMOD W=4u L=1u");
            let _ = writeln!(b, "MAL{r}_{c} {blr} {wlc} {q} 0 NMOD W=3u L=1u");
            let _ = writeln!(b, "MAR{r}_{c} {blbr} {wlc} {qb} 0 NMOD W=3u L=1u");
            let _ = writeln!(b, "Cq{r}_{c} {q} 0 2f");
            let _ = writeln!(b, "Cqb{r}_{c} {qb} 0 2f");
            // 1 Meg tilt resistors hint the stored checkerboard pattern so that both simulators find the same DC state
            let one = (r + c) % 2 == 0;
            let _ = writeln!(b, "Rt{r}_{c} {q} {} 1Meg", if one { "vdd" } else { "0" });
            let _ = writeln!(b, "Rtb{r}_{c} {qb} {} 1Meg", if one { "0" } else { "vdd" });
            let _ = writeln!(b, "Rbl{c}_{r} {blr} bl{c}_{} 10", r + 1);
            let _ = writeln!(b, "Cbl{c}_{r} {blr} 0 2f");
            let _ = writeln!(b, "Rblb{c}_{r} {blbr} blb{c}_{} 10", r + 1);
            let _ = writeln!(b, "Cblb{c}_{r} {blbr} 0 2f");
            let _ = writeln!(b, "Rwl{r}_{c} {wlc} wl{r}_{} 10", c + 1);
            let _ = writeln!(b, "Cwl{r}_{c} {wlc} 0 1f");
        }
    }
    for c in 0..cols {
        let _ = writeln!(b, "Rbt{c} bl{c}_{rows} 0 1Meg");
        let _ = writeln!(b, "Rbtb{c} blb{c}_{rows} 0 1Meg");
    }
    for r in 0..rows {
        let _ = writeln!(b, "Rwt{r} wl{r}_{cols} 0 1Meg");
    }
    b.push_str(if tran {
        ".tran 20p 2n\n.end\n"
    } else {
        ".op\n.end\n"
    });
    b
}

fn sram_dim(n: u64) -> u64 {
    5 * n * n + 5 * n + 2
}

fn gen_sram6t_op(n: u64, _seed: u64) -> GeneratedDeck {
    let mut meta = base_meta(
        format!("fam_sram6t_op_{n}"),
        "sram6t_op",
        format!(
            "{n}x{n} 6T SRAM-like array ({} MOS), checkerboard state, .op",
            6 * n * n
        ),
        sram_dim(n),
        est_rss("sram6t_op", n),
    );
    meta.probes = vec![
        "v(q0_0)".into(),
        "v(qb0_0)".into(),
        format!("v(q{}_{})", n - 1, n - 1),
    ];
    meta.tol_rel = Some(1e-5);
    meta.tol_abs = Some(1e-7);
    if n >= 48 {
        meta.ng_solver = Some("klu");
    }
    meta.tags.extend([
        "nonlinear".into(),
        "mos".into(),
        "bistable".into(),
        "scaling".into(),
    ]);
    meta.note = Some("bistable cells, 1 Meg tilt resistors select the stored pattern; audit: RustSpice n=24 41 ms, n=32 0.7 s/138 MB, n=48 6.2 s/566 MB (tier switch, faer-nonlinear-slow vs KLU 0.46 s); ngspice SPARSE n=48 2.5-24 s".into());
    finish(
        meta,
        "sram6t_op",
        &format!("sram6t_op n={n}"),
        sram_body(n, false),
    )
}

fn gen_sram6t_tran(n: u64, _seed: u64) -> GeneratedDeck {
    let mut meta = base_meta(
        format!("fam_sram6t_tran_{n}"),
        "sram6t_tran",
        format!("{n}x{n} 6T SRAM-like array, word-line pulse on row 0, .tran 20p 2n"),
        sram_dim(n),
        est_rss("sram6t_tran", n),
    );
    // cell (0,0) stores 1 (q high): reading discharges the complementary bit line; q0_0 itself barely moves (vacuous, dropped)
    meta.probes = vec!["v(qb0_0)".into(), "v(blb0_0)".into(), "v(wl0_1)".into()];
    // calibrated on n=8: 2.3 % of swing on the disturbed storage node
    tran_meta_common(&mut meta, 5e-2, 1e-2, n >= 16);
    if n >= 24 {
        meta.ng_solver = Some("klu");
    }
    meta.tags.extend([
        "nonlinear".into(),
        "mos".into(),
        "bistable".into(),
        "tran".into(),
        "scaling".into(),
    ]);
    let mut note = String::from("100 steps, read of row 0; audit: RustSpice n=16 0.1-0.6 s, n=24 0.35-1.8 s; ngspice SPARSE n=24 1.8-11.8 s");
    if n >= 32 {
        // n = 48, measured with 7bb6d36: RustSpice 166 s wall / 1230 s cpu (faer tier, parallel), 605 MiB; ngspice-KLU 4.2 s (same answer)
        // n = 32, measured with the same build: RustSpice 36 s wall on one pinned core at host load 7-14 (no result within
        // 120 s at load 25-28; 50 s wall / 200 s cpu unpinned at load ~30), ngspice-KLU 0.7 s (8 s at load 30)
        meta.known_gap = Some("faer-nonlinear-slow");
        if n >= 48 {
            meta.gap_note = Some("RustSpice needs 166 s wall (1230 s cpu) for the 100-step transient of the 48x48 array (605 MiB) against 4.2 s for ngspice-KLU, which gives the same waveform: the faer tier is far slower than KLU on nonlinear systems; counts as a timeout at the default case timeout".into());
            note.push_str("; RustSpice 166 s, ngspice-KLU 4.2 s");
        } else {
            meta.gap_note = Some("RustSpice needs 36 s wall on one pinned core (more under host load; 200 cpu-s when unpinned) for the 100-step transient of the 32x32 array (5282 unknowns) against 0.7 s for ngspice-KLU: the faer tier is far slower than KLU on nonlinear systems".into());
            note.push_str("; RustSpice 36 s pinned, ngspice-KLU 0.7 s");
        }
    }
    meta.note = Some(note);
    finish(
        meta,
        "sram6t_tran",
        &format!("sram6t_tran n={n}"),
        sram_body(n, true),
    )
}

// ------------------------------------------------------------------------------------------------ star hub

/// Exact `(v_hub, v_leaf)` of the star: `V1 in 0 1; Rs in hub 50; k x (Rl hub l_i 100; Rg l_i 0 1k)`.
pub fn star_exact(k: usize) -> (f64, f64) {
    let leaf = 1100.0 / k as f64;
    let vhub = leaf / (50.0 + leaf);
    (vhub, vhub * 1000.0 / 1100.0)
}

fn gen_star_hub(k: u64, _seed: u64) -> GeneratedDeck {
    let dim = k + 3;
    let mut meta = base_meta(
        format!("fam_star_hub_{k}"),
        "star_hub",
        format!("star hub, {k} resistive leaves (100 ohm + 1k), .op"),
        dim,
        est_rss("star_hub", k),
    );
    meta.probes = vec![
        "v(hub)".into(),
        "v(l1)".into(),
        format!("v(l{k})"),
        "i(v1)".into(),
    ];
    meta.tol_rel = Some(1e-6);
    meta.tol_abs = Some(1e-9);
    if dim <= super::oracle::DC_DENSE_LIMIT as u64 {
        meta.oracle = Some("dc_linear".into());
    }
    meta.tags
        .extend(["linear".into(), "scaling".into(), "dense_fill".into()]);
    let mut note = String::from("one hub node fed through 50 ohm drives k leaves; exact closed form in families.rs (star_exact); RustSpice k=1000 16 ms/5 MB and k=1800 0.02 s/18 MiB, then the cliff at dim 2003 (k=2000) where it is still growing at 7.1 GiB after 46 s; ngspice 0.05-0.14 s");
    if k == 2000 {
        note.push_str("; est_rss above the default --max-rss-mib: large scale only");
    }
    meta.note = Some(note);
    let mut b = String::new();
    b.push_str("V1 in 0 DC 1\nRs in hub 50\n");
    for i in 1..=k {
        let _ = writeln!(b, "Rl{i} hub l{i} 100");
        let _ = writeln!(b, "Rg{i} l{i} 0 1k");
    }
    b.push_str(".op\n.end\n");
    finish(meta, "star_hub", &format!("star_hub k={k}"), b)
}

// ------------------------------------------------------------------------------------------------ parse stress

#[derive(Clone, Copy, PartialEq, Eq)]
enum Flavor {
    Res,
    Diode,
    Subckt,
}

/// Parse-stress decks: `lines` total lines, ~1000 unknowns at most, so that parse + compile dominate. Resistor and
/// subckt flavours are a 1-D chain of `k` nodes with ~`(lines-k)/k` parallel elements per stage and a shunt chosen
/// for decay length k/5 (all nodes O(1) V); the diode flavour biases `k` nodes from a rail through 10k with
/// `(lines-2k)/k` parallel diodes each.
fn gen_parse_stress(flavor: Flavor, lines: u64, _seed: u64) -> GeneratedDeck {
    let (fam, what) = match flavor {
        Flavor::Res => ("parse_stress_res", "resistor-heavy"),
        Flavor::Diode => ("parse_stress_diode", "diode-heavy"),
        Flavor::Subckt => ("parse_stress_subckt", "subckt-instance-heavy"),
    };
    let l = lines as usize;
    let k = (l / 50).clamp(20, 1000);
    let mut meta = base_meta(
        format!("fam_{fam}_{lines}"),
        fam,
        format!("{what} deck of {lines} lines, .op"),
        lines,
        0,
    );
    // measured: see est_parse() — per-line RustSpice parse+compile memory
    meta.est_rss_mib = est_rss(fam, lines);
    meta.tags
        .extend(["parse".into(), "linear_solve_trivial".into()]);
    let mut b = String::with_capacity(l * 28 + 256);
    match flavor {
        Flavor::Res | Flavor::Subckt => {
            let e = l.saturating_sub(k + 8); // chain-element lines
            let per = (e / k).max(1);
            // parallel values cycle over a fixed set; series conductance per stage is exact
            let vals = [1000.0, 2200.0, 4700.0, 10000.0];
            let gs: f64 = if flavor == Flavor::Res {
                (0..per).map(|i| 1.0 / vals[i % 4]).sum()
            } else {
                per as f64 / 1000.0 // each X cell = two 2 kohm resistors in parallel
            };
            let rs = 1.0 / gs;
            let rsh = (k as f64 / 5.0).powi(2) * rs;
            meta.tol_rel = Some(1e-6);
            meta.tol_abs = Some(1e-9);
            meta.probes = vec![
                "v(n1)".into(),
                format!("v(n{})", k / 4),
                format!("v(n{})", k / 2),
                format!("v(n{k})"),
                "i(v1)".into(),
            ];
            if flavor == Flavor::Res && (k as u64 + 2) <= super::oracle::DC_DENSE_LIMIT as u64 {
                meta.oracle = Some("dc_linear".into());
            }
            meta.note = Some(format!(
                "{k} chain nodes, {per} parallel elements per stage (series {} ohm), shunt {} ohm; solve is trivial, time = read + parse + compile",
                num(rs),
                num(rsh)
            ));
            if flavor == Flavor::Subckt {
                b.push_str(".subckt rcell a b\nR1 a b 2000\nR2 a b 2000\n.ends rcell\n");
            }
            b.push_str("V1 n0 0 DC 1\n");
            let mut emitted = 0usize;
            'outer: for stage in 0..k {
                let cnt = if stage + 1 == k { e - emitted } else { per }.min(e - emitted);
                for i in 0..cnt {
                    if flavor == Flavor::Res {
                        let _ =
                            writeln!(b, "R{emitted} n{stage} n{} {}", stage + 1, num(vals[i % 4]));
                    } else {
                        let _ = writeln!(b, "X{emitted} n{stage} n{} rcell", stage + 1);
                    }
                    emitted += 1;
                    if emitted >= e {
                        break 'outer;
                    }
                }
            }
            for s in 1..=k {
                let _ = writeln!(b, "Rg{s} n{s} 0 {}", num(rsh.round()));
            }
        }
        Flavor::Diode => {
            let per = (l.saturating_sub(2 * k + 8) / k).max(1);
            meta.tol_rel = Some(1e-5);
            meta.tol_abs = Some(1e-8);
            meta.probes = vec![
                "v(n1)".into(),
                format!("v(n{})", k / 2),
                format!("v(n{k})"),
                "i(v1)".into(),
            ];
            meta.tags.push("nonlinear".into());
            meta.note = Some(format!(
                "{k} nodes biased from a 3 V rail through 10k, {per} parallel diodes (two IS values) per node, 1k coupling resistors; Newton on {k} unknowns, time dominated by parse and diode evaluation"
            ));
            b.push_str(
                ".model DMA D(IS=1e-14 N=1.0)\n.model DMB D(IS=2e-14 N=1.0)\nV1 vcc 0 DC 3\n",
            );
            for s in 1..=k {
                let _ = writeln!(b, "Rf{s} vcc n{s} 10k");
                if s > 1 {
                    let _ = writeln!(b, "Rc{s} n{} n{s} 1k", s - 1);
                }
                for i in 0..per {
                    let _ = writeln!(
                        b,
                        "D{s}_{i} n{s} 0 {}",
                        if i % 2 == 0 { "DMA" } else { "DMB" }
                    );
                }
            }
        }
    }
    b.push_str(".op\n.end\n");
    finish(meta, fam, &format!("{fam} lines={lines}"), b)
}

// ------------------------------------------------------------------------------------------------ startup floor

fn gen_startup_floor(_n: u64, _seed: u64) -> GeneratedDeck {
    let mut meta = base_meta(
        "fam_startup_floor_1".to_string(),
        "startup_floor",
        "start-up floor: one resistor, .op".to_string(),
        2,
        est_rss("startup_floor", 1),
    );
    meta.probes = vec!["v(in)".into(), "i(v1)".into()];
    meta.tol_rel = Some(1e-9);
    meta.tol_abs = Some(1e-12);
    meta.oracle = Some("dc_linear".into());
    meta.tags.extend(["startup".into()]);
    meta.note = Some("measures process start-up + parse + output of a trivial deck (ngspice ~3.5-5.5 ms, RustSpice ~1 ms, audit a1)".into());
    finish(
        meta,
        "startup_floor",
        "startup floor",
        "V1 in 0 DC 1\nR1 in 0 1k\n.op\n.end\n".to_string(),
    )
}

// ------------------------------------------------------------------------------------------------ registry

struct Fam {
    name: &'static str,
    small: &'static [u64],
    medium: &'static [u64],
    large: &'static [u64],
    make: fn(u64, u64) -> GeneratedDeck,
}

fn ps_res(n: u64, s: u64) -> GeneratedDeck {
    gen_parse_stress(Flavor::Res, n, s)
}
fn ps_diode(n: u64, s: u64) -> GeneratedDeck {
    gen_parse_stress(Flavor::Diode, n, s)
}
fn ps_subckt(n: u64, s: u64) -> GeneratedDeck {
    gen_parse_stress(Flavor::Subckt, n, s)
}

const FAMILIES: &[Fam] = &[
    Fam {
        name: "startup_floor",
        small: &[1],
        medium: &[],
        large: &[],
        make: gen_startup_floor,
    },
    Fam {
        name: "ladder_dc",
        small: &[200, 1000],
        medium: &[1000, 10_000, 100_000],
        large: &[1_000_000],
        make: gen_ladder_dc,
    },
    Fam {
        name: "mesh_dc",
        small: &[10, 20],
        medium: &[20, 40, 60, 100, 125],
        large: &[150, 200, 250],
        make: gen_mesh_dc,
    },
    Fam {
        name: "mesh_dc_shuffled",
        small: &[10, 20],
        medium: &[30, 40],
        large: &[60],
        make: gen_mesh_dc_shuffled,
    },
    Fam {
        name: "rc_ladder_tran",
        small: &[100],
        medium: &[1000, 10_000],
        large: &[100_000],
        make: gen_rc_ladder_tran,
    },
    Fam {
        name: "rc_mesh_tran",
        small: &[10],
        medium: &[20, 60, 100],
        large: &[140],
        make: gen_rc_mesh_tran,
    },
    Fam {
        name: "lc_ladder_tran",
        small: &[10],
        medium: &[10, 100, 300],
        large: &[1000],
        make: gen_lc_ladder_tran,
    },
    Fam {
        name: "diode_chain",
        small: &[10],
        medium: &[10, 50, 500],
        large: &[],
        make: gen_diode_chain,
    },
    Fam {
        name: "diode_mesh",
        small: &[8],
        medium: &[16, 48],
        large: &[100],
        make: gen_diode_mesh,
    },
    Fam {
        name: "cmos_chain_op",
        small: &[5],
        medium: &[5, 50, 100, 150],
        large: &[200, 300, 400, 2000],
        make: gen_cmos_chain_op,
    },
    Fam {
        name: "cmos_chain_tran",
        small: &[5],
        medium: &[5, 50, 100, 150],
        large: &[200, 300, 400, 2000],
        make: gen_cmos_chain_tran,
    },
    Fam {
        name: "mos_array_op",
        small: &[5],
        medium: &[10, 20, 30],
        large: &[50, 71],
        make: gen_mos_array_op,
    },
    Fam {
        name: "mos_array_tran",
        small: &[20],
        medium: &[10, 30],
        large: &[50, 71],
        make: gen_mos_array_tran,
    },
    Fam {
        name: "mos_array_tran_fast",
        small: &[5],
        medium: &[],
        large: &[],
        make: gen_mos_array_tran_fast,
    },
    Fam {
        name: "sram6t_op",
        small: &[8],
        medium: &[16, 24],
        large: &[32, 48],
        make: gen_sram6t_op,
    },
    Fam {
        name: "sram6t_tran",
        small: &[8],
        medium: &[16, 24],
        large: &[32, 48],
        make: gen_sram6t_tran,
    },
    Fam {
        name: "star_hub",
        small: &[100],
        medium: &[100, 1000, 1800],
        large: &[2000],
        make: gen_star_hub,
    },
    Fam {
        name: "parse_stress_res",
        small: &[1000],
        medium: &[1000, 10_000, 100_000],
        large: &[1_000_000],
        make: ps_res,
    },
    Fam {
        name: "parse_stress_diode",
        small: &[1000],
        medium: &[1000, 10_000, 100_000],
        large: &[1_000_000],
        make: ps_diode,
    },
    Fam {
        name: "parse_stress_subckt",
        small: &[1000],
        medium: &[1000, 10_000, 100_000],
        large: &[1_000_000],
        make: ps_subckt,
    },
];

/// Sizes generated for `family` at `scale` (cumulative, ascending, de-duplicated).
pub fn sizes(family: &str, scale: Scale) -> Vec<u64> {
    let mut v: Vec<u64> = Vec::new();
    if let Some(f) = FAMILIES.iter().find(|f| f.name == family) {
        v.extend_from_slice(f.small);
        if scale >= Scale::Medium {
            v.extend_from_slice(f.medium);
        }
        if scale >= Scale::Large {
            v.extend_from_slice(f.large);
        }
    }
    v.sort_unstable();
    v.dedup();
    v
}

/// Names of all scaling families (generation order).
pub fn family_names() -> Vec<&'static str> {
    FAMILIES.iter().map(|f| f.name).collect()
}

/// All scaling families for `scale`, deterministic for a given `seed` (see the module documentation for the lists).
pub fn generate(scale: Scale, seed: u64) -> Vec<GeneratedDeck> {
    let mut out = Vec::new();
    for f in FAMILIES {
        for n in sizes(f.name, scale) {
            out.push((f.make)(n, seed));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bench::oracle;
    use crate::bench::oracle::testutil;

    fn meta_of(deck: &str, key: &str) -> Option<String> {
        let pat = format!("* bench.{key}:");
        deck.lines()
            .find_map(|l| l.strip_prefix(&pat).map(|v| v.trim().to_string()))
    }

    #[test]
    fn rng_is_deterministic_and_seed_sensitive() {
        let a: Vec<u64> = (0..8)
            .map({
                let mut r = Rng::new(7);
                move |_| r.next_u64()
            })
            .collect();
        let b: Vec<u64> = (0..8)
            .map({
                let mut r = Rng::new(7);
                move |_| r.next_u64()
            })
            .collect();
        let c: Vec<u64> = (0..8)
            .map({
                let mut r = Rng::new(8);
                move |_| r.next_u64()
            })
            .collect();
        assert_eq!(a, b);
        assert_ne!(a, c);
        let mut r = Rng::new(1);
        for _ in 0..1000 {
            let f = r.next_f64();
            assert!((0.0..1.0).contains(&f));
            assert!(r.below(5) < 5);
        }
    }

    #[test]
    fn generate_is_deterministic_and_seed_changes_only_shuffles() {
        let a = generate(Scale::Small, 1);
        let b = generate(Scale::Small, 1);
        assert_eq!(a.len(), b.len());
        for (x, y) in a.iter().zip(&b) {
            assert_eq!(x.deck, y.deck);
            assert_eq!(x.id, y.id);
        }
        let c = generate(Scale::Small, 2);
        let mut differing = Vec::new();
        for (x, y) in a.iter().zip(&c) {
            if x.deck != y.deck {
                differing.push(x.family.clone());
            }
        }
        assert!(
            !differing.is_empty() && differing.iter().all(|f| f == "mesh_dc_shuffled"),
            "{differing:?}"
        );
    }

    #[test]
    fn metadata_is_complete_and_consistent() {
        for scale in [Scale::Small, Scale::Medium] {
            let decks = generate(scale, 1);
            let mut ids = std::collections::BTreeSet::new();
            for d in &decks {
                assert!(ids.insert(d.id.clone()), "duplicate id {}", d.id);
                assert!(
                    d.id.starts_with("fam_")
                        && d.id
                            .chars()
                            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'),
                    "{}",
                    d.id
                );
                assert_eq!(meta_of(&d.deck, "id").as_deref(), Some(d.id.as_str()));
                assert_eq!(meta_of(&d.deck, "group").as_deref(), Some("performance"));
                assert_eq!(meta_of(&d.deck, "timing").as_deref(), Some("yes"));
                assert_eq!(meta_of(&d.deck, "expect").as_deref(), Some("match"));
                assert_eq!(meta_of(&d.deck, "size").unwrap(), d.size.to_string());
                assert_eq!(
                    meta_of(&d.deck, "est_rss_mib").unwrap(),
                    d.est_rss_mib.to_string()
                );
                assert!(
                    meta_of(&d.deck, "tags")
                        .unwrap()
                        .contains(&format!("family:{}", d.family)),
                    "{}",
                    d.id
                );
                assert!(d.est_rss_mib >= 8 && d.size >= 1);
                // exactly one analysis card, ends with .end, no .control
                let lower = d.deck.to_ascii_lowercase();
                let analyses = lower
                    .lines()
                    .filter(|l| {
                        l.starts_with(".op") || l.starts_with(".dc") || l.starts_with(".tran")
                    })
                    .count();
                assert_eq!(analyses, 1, "{}", d.id);
                assert!(lower.trim_end().ends_with(".end"), "{}", d.id);
                assert!(!lower.contains(".control"));
                assert!(d.deck.is_ascii(), "{}", d.id);
                if let Some(g) = meta_of(&d.deck, "known_gap") {
                    assert!(
                        crate::bench::meta::KNOWN_GAP_SLUGS.contains(&g.as_str()),
                        "{}: slug {g}",
                        d.id
                    );
                    assert!(meta_of(&d.deck, "gap.note").is_some());
                }
                // tran decks have tol.norm, op/dc decks have tol.rel/abs
                if lower.lines().any(|l| l.starts_with(".tran")) {
                    assert!(meta_of(&d.deck, "tol.norm").is_some(), "{}", d.id);
                } else {
                    assert!(
                        meta_of(&d.deck, "tol.rel").is_some()
                            && meta_of(&d.deck, "tol.abs").is_some(),
                        "{}",
                        d.id
                    );
                }
                // all probes are lower case
                for p in meta_of(&d.deck, "probes").unwrap_or_default().split(',') {
                    let p = p.trim();
                    assert_eq!(p, p.to_ascii_lowercase());
                }
            }
        }
    }

    #[test]
    fn scale_lists_match_the_documented_table() {
        assert_eq!(sizes("ladder_dc", Scale::Small), vec![200, 1000]);
        assert_eq!(
            sizes("ladder_dc", Scale::Medium),
            vec![200, 1000, 10_000, 100_000]
        );
        assert_eq!(
            sizes("ladder_dc", Scale::Large),
            vec![200, 1000, 10_000, 100_000, 1_000_000]
        );
        assert_eq!(
            sizes("mesh_dc", Scale::Medium),
            vec![10, 20, 40, 60, 100, 125]
        );
        assert_eq!(
            sizes("mesh_dc_shuffled", Scale::Large),
            vec![10, 20, 30, 40, 60]
        );
        assert_eq!(
            sizes("cmos_chain_tran", Scale::Large),
            vec![5, 50, 100, 150, 200, 300, 400, 2000]
        );
        assert_eq!(sizes("star_hub", Scale::Large), vec![100, 1000, 1800, 2000]);
        assert_eq!(sizes("startup_floor", Scale::Small), vec![1]);
        let counts: Vec<usize> = [Scale::Small, Scale::Medium]
            .iter()
            .map(|s| generate(*s, 1).len())
            .collect();
        // 23 since `mos_array_tran_fast` joined the families (the count was 22 before)
        assert_eq!(counts[0], 23, "small deck count");
        assert!(counts[1] > counts[0]);
    }

    #[test]
    fn est_rss_bounds_every_measured_peak_from_above() {
        // every deck of the large ladder whose (family, size) was measured declares at least 1.25 x the measured peak:
        // the engine's skipped_budget decision and the report's memory column rely on est being an upper bound
        let all = generate(Scale::Large, 1);
        let mut checked = 0;
        for d in &all {
            let size: u64 = d.id.rsplit('_').next().unwrap().parse().unwrap();
            let fam = d.id.strip_prefix("fam_").unwrap();
            let fam = &fam[..fam.len() - size.to_string().len() - 1];
            if let Some((_, t)) = PEAK_MIB.iter().find(|(f, _)| *f == fam) {
                if let Some((_, peak)) = t.iter().find(|(s, _)| *s == size) {
                    assert!(
                        d.est_rss_mib as f64 >= 1.25 * *peak as f64,
                        "{}: est {} < 1.25 x measured {}",
                        d.id,
                        d.est_rss_mib,
                        peak
                    );
                    checked += 1;
                }
            }
        }
        assert!(checked >= 80, "only {checked} decks matched the table");
        // log-log interpolation is monotone between table points and passes through them
        assert_eq!(interp(&[(10, 100), (1000, 10_000)], 100.0).round(), 1000.0);
    }

    #[test]
    fn known_gaps_only_where_the_audit_evidenced_them() {
        let all = generate(Scale::Large, 1);
        let gap = |id: &str| {
            let d = all
                .iter()
                .find(|d| d.id == id)
                .unwrap_or_else(|| panic!("{id}"));
            meta_of(&d.deck, "known_gap")
        };
        assert_eq!(gap("fam_mesh_dc_shuffled_60"), None);
        assert_eq!(gap("fam_mesh_dc_shuffled_40"), None);
        assert_eq!(gap("fam_lc_ladder_tran_1000"), None);
        assert_eq!(gap("fam_lc_ladder_tran_300"), None);
        assert_eq!(gap("fam_star_hub_2000"), None);
        assert_eq!(gap("fam_cmos_chain_tran_200"), None);
        assert_eq!(gap("fam_cmos_chain_tran_150"), None);
        assert_eq!(gap("fam_cmos_chain_op_300"), None);
        assert_eq!(gap("fam_cmos_chain_op_400"), None);
        assert_eq!(gap("fam_mesh_dc_250"), None);
        // MOS arrays: size-dependent, each measured with the shipped binary (see mos_array_gap)
        assert_eq!(gap("fam_mos_array_op_10"), None);
        for id in [
            "fam_mos_array_op_5",
            "fam_mos_array_op_20",
            "fam_mos_array_op_30",
            "fam_mos_array_op_50",
            "fam_mos_array_op_71",
            "fam_mos_array_tran_20",
            "fam_mos_array_tran_30",
            "fam_mos_array_tran_50",
        ] {
            assert_eq!(gap(id), None, "{id}");
        }
        assert_eq!(gap("fam_mos_array_tran_10"), None);
        assert_eq!(
            gap("fam_mos_array_tran_71").as_deref(),
            Some("faer-nonlinear-slow")
        );
        // the SRAM transient crosses the same cliff between 24x24 and 32x32 (large scale: 0.7 s for KLU, 36 s for RustSpice)
        assert_eq!(gap("fam_sram6t_tran_24"), None);
        for id in ["fam_sram6t_tran_32", "fam_sram6t_tran_48"] {
            assert_eq!(gap(id).as_deref(), Some("faer-nonlinear-slow"), "{id}");
        }
        // RSS estimates of the documented heavy cases are above the default 4 GiB cap, the others are below
        let est = |id: &str| all.iter().find(|d| d.id == id).unwrap().est_rss_mib;
        assert!(est("fam_mesh_dc_shuffled_60") > 4096);
        assert!(est("fam_lc_ladder_tran_1000") > 4096);
        assert!(est("fam_star_hub_2000") > 4096);
        assert!(
            est("fam_star_hub_1800") < 100,
            "star k=1800 is below the cliff: 18 MiB measured"
        );
        // measured peaks (PEAK_MIB): 778, 2411, 525, 217 MiB; est = 1.25 x peak + 8
        assert!(est("fam_mesh_dc_100") < 4096 && est("fam_mesh_dc_100") >= 980);
        assert!(est("fam_ladder_dc_1000000") >= 3020 && est("fam_ladder_dc_1000000") < 4096);
        assert!(est("fam_rc_ladder_tran_100000") >= 660 && est("fam_rc_ladder_tran_100000") < 800);
        assert!(est("fam_mesh_dc_250") < 400);
        // the three 7 GiB-class decks were measured to be at least this large under a 3 GiB cap, so est is an upper bound
        assert!(est("fam_star_hub_2000") >= 9000);
        // under-estimates found by the engine run: mos_array_op_50/71, tran_50/71 and sram6t_tran_48 (605 MiB)
        assert!(est("fam_mos_array_op_50") >= 600 && est("fam_mos_array_tran_71") >= 710);
        assert!(est("fam_sram6t_tran_48") >= 760);
    }

    #[test]
    fn ladder_exact_matches_closed_form_and_oracle() {
        // two stages by hand: rs=1, rsh=R: Z2=R, Z1=R||(1+R); v1=Z1/(1+Z1), v2=v1*R/(1+R)
        let r = 100.0;
        let v = ladder_exact(2, 1.0, r);
        let z1 = r * (1.0 + r) / (r + 1.0 + r);
        let v1 = z1 / (1.0 + z1);
        assert!((v[1] - v1).abs() < 1e-15 && (v[2] - v1 * r / (1.0 + r)).abs() < 1e-15);
        let d = gen_ladder_dc(200, 1);
        let (rs, rsh) = ladder_params(200);
        let ex = ladder_exact(200, rs, rsh);
        for k in [1usize, 50, 100, 150, 200] {
            let o = oracle::evaluate(&d.deck, "dc_linear", &format!("v(n{k})"), &[0.0]).unwrap();
            // the oracle solves the system with 1/rsh rounded to a double: kappa*u ~ 1e-13 relative is inherent
            assert!(
                (o[0] - ex[k]).abs() < 1e-11 * ex[k].abs().max(1e-3),
                "k={k}: {} vs {}",
                o[0],
                ex[k]
            );
        }
        // all nodes O(1): smallest node is 1/cosh(5) = 0.0135 (open far end), not 1e-34
        assert!((ex[200] - 1.0 / 5.0f64.cosh()).abs() < 1e-3, "{}", ex[200]);
        let big = ladder_exact(1_000_000, 1.0, ladder_params(1_000_000).1);
        assert!(big[1_000_000] > 1e-3);
        // current: i(v1) = -(1 - v1)/rs
        let i = oracle::evaluate(&d.deck, "dc_linear", "i(v1)", &[0.0]).unwrap()[0];
        let want = (1.0 - ex[1]) / rs;
        assert!((i + want).abs() < 1e-9 * want.abs(), "{i} vs {}", -want);
    }

    #[test]
    fn star_exact_matches_oracle() {
        let d = gen_star_hub(100, 1);
        let (vh, vl) = star_exact(100);
        let a = oracle::evaluate(&d.deck, "dc_linear", "v(hub)", &[0.0]).unwrap()[0];
        let b = oracle::evaluate(&d.deck, "dc_linear", "v(l100)", &[0.0]).unwrap()[0];
        assert!(
            (a - vh).abs() < 1e-14 && (b - vl).abs() < 1e-14,
            "{a} {vh} {b} {vl}"
        );
    }

    #[test]
    fn mesh_oracle_agrees_with_pad_symmetry_and_kcl() {
        // pad voltage drop: total tap current 90 mA through four 0.01 ohm pads: v(corner) < 1 and symmetric corners
        let d = gen_mesh_dc(10, 1);
        let get = |n: &str| oracle::evaluate(&d.deck, "dc_linear", n, &[0.0]).unwrap()[0];
        let c00 = get("v(n0_0)");
        let c09 = get("v(n0_9)");
        assert!(c00 < 1.0 && c00 > 0.9, "{c00}");
        assert!((c00 - c09).abs() < 1e-3);
        let i = get("i(v1)");
        assert!(
            (i + 0.09).abs() < 1e-12,
            "total tap current must flow out of V1: {i}"
        );
    }

    #[test]
    fn shuffled_mesh_is_the_same_circuit_as_natural_mesh() {
        let a = gen_mesh_dc(10, 1);
        let b = gen_mesh_dc_shuffled(10, 1);
        let body = |d: &GeneratedDeck| {
            let mut v: Vec<String> = d
                .deck
                .lines()
                .filter(|l| {
                    !l.starts_with('*')
                        && !l.starts_with(".op")
                        && !l.starts_with(".end")
                        && !l.starts_with("mesh")
                })
                .map(|s| s.to_string())
                .collect();
            v.sort();
            v
        };
        assert_eq!(body(&a), body(&b));
        assert_ne!(
            a.deck.lines().collect::<Vec<_>>(),
            b.deck.lines().collect::<Vec<_>>()
        );
        let na = oracle::evaluate(&a.deck, "dc_linear", "v(n5_5)", &[0.0]).unwrap()[0];
        let nb = oracle::evaluate(&b.deck, "dc_linear", "v(n5_5)", &[0.0]).unwrap()[0];
        assert!((na - nb).abs() < 1e-13);
        // first-appearance order really differs: the natural deck starts with V1
        let first = |d: &GeneratedDeck| {
            d.deck
                .lines()
                .find(|l| l.starts_with('V') || l.starts_with('R') || l.starts_with('I'))
                .unwrap()
                .to_string()
        };
        assert!(first(&a).starts_with("V1"));
        assert!(!first(&b).starts_with("V1 vdd"));
    }

    #[test]
    fn oracles_attached_to_small_decks_evaluate() {
        // every oracle named in a Small deck must be able to evaluate all of its probes (cheap: Small only)
        for d in generate(Scale::Small, 1) {
            let Some(spec) = meta_of(&d.deck, "oracle") else {
                continue;
            };
            let probes = meta_of(&d.deck, "probes").unwrap();
            let axis: Vec<f64> = if spec == "tran_lti" {
                (0..=20)
                    .map(|k| {
                        5e-9 * k as f64 / 20.0
                            * if d.family == "lc_ladder_tran" {
                                4.0
                            } else {
                                1.0
                            }
                    })
                    .collect()
            } else if d.deck.lines().any(|l| l.starts_with(".dc")) {
                let t: Vec<&str> = d
                    .deck
                    .lines()
                    .find(|l| l.starts_with(".dc"))
                    .unwrap()
                    .split_whitespace()
                    .collect();
                let (a, b, s): (f64, f64, f64) = (
                    oracle::parse_number(t[2]).unwrap(),
                    oracle::parse_number(t[3]).unwrap(),
                    oracle::parse_number(t[4]).unwrap(),
                );
                let n = ((b - a) / s).round() as usize;
                (0..=n).map(|k| a + s * k as f64).collect()
            } else {
                vec![0.0]
            };
            for p in probes.split(',') {
                let t0 = std::time::Instant::now();
                let r = oracle::evaluate(&d.deck, &spec, p.trim(), &axis);
                eprintln!("{} {p}: {:.2}s", d.id, t0.elapsed().as_secs_f64());
                let r = r.unwrap_or_else(|e| panic!("{} {p}: {e}", d.id));
                assert!(r.iter().all(|v| v.is_finite()));
            }
        }
    }

    #[test]
    fn generation_memory_is_bounded() {
        let total = |sc: Scale| generate(sc, 1).iter().map(|d| d.deck.len()).sum::<usize>();
        let small = total(Scale::Small);
        let medium = total(Scale::Medium);
        assert!(small < 4 << 20, "small scale text: {small} bytes");
        assert!(medium < 64 << 20, "medium scale text: {medium} bytes");
        assert!(medium > small);
    }

    /// Oracle versus ngspice-47 (black box, tight options) on every oracle-carrying deck of `H2_SCALE` (default small),
    /// on ngspice's own output axis; prints the swing-normalised error per probe:
    /// `cargo test --release ... oracle_vs_ngspice -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn oracle_vs_ngspice() {
        use crate::bench::oracle::testutil::{run_ngspice, tight};
        let sc = match std::env::var("H2_SCALE").as_deref() {
            Ok("medium") => Scale::Medium,
            _ => Scale::Small,
        };
        let mut worst = 0.0f64;
        for d in generate(sc, 1) {
            let Some(spec) = meta_of(&d.deck, "oracle") else {
                continue;
            };
            let Some(probes) = meta_of(&d.deck, "probes") else {
                continue;
            };
            let raw = match run_ngspice(&tight(&d.deck), &d.id) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("{:34} ngspice failed: {e}", d.id);
                    continue;
                }
            };
            let axis = if raw.plot.to_lowercase().contains("operating") {
                vec![0.0]
            } else {
                raw.cols[0].clone()
            };
            for p in probes.split(',') {
                let p = p.trim();
                let Some(ng) = raw.col(p) else {
                    eprintln!("{:34} {p}: not in ngspice output", d.id);
                    continue;
                };
                match oracle::evaluate(&d.deck, &spec, p, &axis) {
                    Ok(o) => {
                        let hi = ng.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                        let lo = ng.iter().cloned().fold(f64::INFINITY, f64::min);
                        let amax = hi.abs().max(lo.abs());
                        let den = if spec == "tran_lti" {
                            (hi - lo).max(1e-12 * amax)
                        } else {
                            amax
                        }
                        .max(1e-300);
                        let err = o
                            .iter()
                            .zip(ng)
                            .fold(0.0f64, |m, (a, b)| m.max((a - b).abs()))
                            / den;
                        eprintln!(
                            "{:34} {spec:12} {p:10} pts={:5} norm_err={err:.3e}",
                            d.id,
                            axis.len()
                        );
                        worst = worst.max(err);
                    }
                    Err(e) => eprintln!("{:34} {p}: oracle error {e}", d.id),
                }
            }
        }
        eprintln!("worst normalised error {worst:e}");
    }

    /// Wall-clock of the transient oracle on the family decks that carry one, with a dense (10 000 point) axis like the
    /// engine's reference grid; meaningful only in release: `cargo test --release ... -- --ignored oracle_timing`.
    #[test]
    #[ignore]
    fn oracle_timing() {
        for d in generate(Scale::Small, 1) {
            if meta_of(&d.deck, "oracle").as_deref() != Some("tran_lti") {
                continue;
            }
            let t: Vec<&str> = d
                .deck
                .lines()
                .find(|l| l.starts_with(".tran"))
                .unwrap()
                .split_whitespace()
                .collect();
            let tstop = oracle::parse_number(t[2]).unwrap();
            let axis: Vec<f64> = (0..=10_000).map(|k| tstop * k as f64 / 10_000.0).collect();
            for p in meta_of(&d.deck, "probes").unwrap().split(',') {
                let t0 = std::time::Instant::now();
                let r = oracle::evaluate(&d.deck, "tran_lti", p.trim(), &axis);
                eprintln!(
                    "{} {p}: {} in {:.2}s",
                    d.id,
                    if r.is_ok() { "ok" } else { "ERR" },
                    t0.elapsed().as_secs_f64()
                );
            }
        }
    }

    /// Prints the inventory (deck count, sizes, est_rss_mib, known gaps) per scale: `-- --ignored --nocapture inventory`.
    #[test]
    #[ignore]
    fn inventory() {
        for sc in [Scale::Small, Scale::Medium, Scale::Large] {
            let decks = generate(sc, 1);
            eprintln!("== {sc:?}: {} decks", decks.len());
            for f in family_names() {
                let v: Vec<String> = decks
                    .iter()
                    .filter(|d| d.family == f)
                    .map(|d| {
                        let gap = meta_of(&d.deck, "known_gap").map(|_| "*").unwrap_or("");
                        format!("{}{gap}/{}MiB", d.size, d.est_rss_mib)
                    })
                    .collect();
                eprintln!("  {f:22} {}", v.join(" "));
            }
        }
    }

    /// Large scale text volume (documented as ~300 MiB peak): `cargo test ... -- --ignored large_scale_volume`.
    #[test]
    #[ignore]
    fn large_scale_volume() {
        let t0 = std::time::Instant::now();
        let decks = generate(Scale::Large, 1);
        let total: usize = decks.iter().map(|d| d.deck.len()).sum();
        let biggest = decks.iter().max_by_key(|d| d.deck.len()).unwrap();
        eprintln!(
            "large: {} decks, {:.1} MiB of text, biggest {} ({:.1} MiB), generated in {:.2}s",
            decks.len(),
            total as f64 / 1048576.0,
            biggest.id,
            biggest.deck.len() as f64 / 1048576.0,
            t0.elapsed().as_secs_f64()
        );
    }

    /// Writes every generated deck of `H2_SCALE` (default medium) to `H2_DUMP_DIR` (one `<id>.sp` each), without running
    /// anything; for external measurement: `H2_DUMP_DIR=/dev/shm/x cargo test --release -p rustspice-cli dump_decks -- --ignored`.
    #[test]
    #[ignore]
    fn dump_decks() {
        let scale =
            Scale::parse(&std::env::var("H2_SCALE").unwrap_or_else(|_| "medium".into())).unwrap();
        let dir = std::path::PathBuf::from(std::env::var("H2_DUMP_DIR").expect("H2_DUMP_DIR"));
        std::fs::create_dir_all(&dir).unwrap();
        for d in generate(scale, 1) {
            std::fs::write(dir.join(format!("{}.sp", d.id)), &d.deck).unwrap();
        }
    }

    /// Writes the decks of `H2_SCALE` (default small) to `H2_DUMP_DIR` (default work dir), runs each through ngspice
    /// (tight options, black box) and, when `RSPROBE` is set, through the RustSpice probe. Prints one line per deck.
    /// Run by hand: `cargo test -p rustspice-cli families::tests::dump_and_check -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn dump_and_check() {
        if !testutil::ngspice_ok() {
            eprintln!("ngspice not available: skipped");
            return;
        }
        let scale =
            Scale::parse(&std::env::var("H2_SCALE").unwrap_or_else(|_| "small".into())).unwrap();
        let filter = std::env::var("H2_FILTER").unwrap_or_default();
        let dir = std::env::var("H2_DUMP_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| testutil::work_dir().join("fam"));
        let _ = std::fs::create_dir_all(&dir);
        for d in generate(scale, 1) {
            if !d.id.contains(&filter) {
                continue;
            }
            let path = dir.join(format!("{}.sp", d.id));
            std::fs::write(&path, &d.deck).unwrap();
            let t0 = std::time::Instant::now();
            let ng_deck = if std::env::var("H2_NG_KLU").is_ok() {
                testutil::with_options(&d.deck, ".options klu")
            } else {
                d.deck.clone()
            };
            let ng = testutil::run_ngspice(&ng_deck, &format!("fam_{}", d.id));
            let ngs = match &ng {
                Ok(r) => format!(
                    "ngspice ok pts={} ({:.2}s)",
                    r.cols[0].len(),
                    t0.elapsed().as_secs_f64()
                ),
                Err(e) => format!("ngspice FAILED: {e}"),
            };
            let rs = match testutil::run_probe(&path) {
                Some((ok, _, err)) => format!("rsprobe {} {}", if ok { "ok" } else { "FAIL" }, err),
                None => "rsprobe n/a".to_string(),
            };
            eprintln!(
                "{:30} size={:7} est={:5} | {ngs} | {rs}",
                d.id, d.size, d.est_rss_mib
            );
        }
    }
}
