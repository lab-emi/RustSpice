//! Batched DC operating points on the batch kernel (`cpu_ref` and CUDA) against the CPU solver's batch API, and
//! against an independent KCL check written from the circuit description (not from the solver's stamps).

use rustspice_backend_cpu::BatchCircuitInput;
use rustspice_backend_cuda_simt::batch::{solve_cpu_ref, status, CudaBatchSolver};
use rustspice_solver::{BatchDcEngine, Simulator};

/// Thermal voltage at 27 C with the CODATA 2014 constants that ngspice uses (`k T / q` it shows black box).
const VT: f64 = 1.380_648_52e-23 * 300.15 / 1.602_176_620_8e-19;
const GMIN: f64 = 1e-12;

#[derive(Clone, Copy)]
struct Diode {
    a: usize,
    c: usize,
    is: f64,
    n: f64,
    area: f64,
}

/// A level-1 MOSFET with its own model card (bulk junctions at the default `IS` = 1e-14 A, no capacitances).
#[derive(Clone, Copy)]
struct Mos {
    d: usize,
    g: usize,
    s: usize,
    b: usize,
    pmos: bool,
    w: f64,
    l: f64,
    vto: f64,
    kp: f64,
    lambda: f64,
    gamma: f64,
    phi: f64,
}

impl Mos {
    /// Drain current (drain to source through the channel) from the SPICE level-1 (Shichman-Hodges) equations:
    /// the device is symmetric (drain and source swap when `vds < 0`), a PMOS is an NMOS with every voltage and the
    /// current negated, and the threshold rises with reverse body bias, linearised in `vbs` for forward bias.
    fn drain_current(&self, vd: f64, vg: f64, vs: f64, vb: f64) -> f64 {
        let p = if self.pmos { -1.0 } else { 1.0 };
        let swap = p * (vd - vs) < 0.0;
        let (vds, vgs, vbs) = if swap {
            (p * (vs - vd), p * (vg - vd), p * (vb - vd))
        } else {
            (p * (vd - vs), p * (vg - vs), p * (vb - vs))
        };
        let body = if vbs <= 0.0 {
            (self.phi - vbs).sqrt() - self.phi.sqrt()
        } else {
            -vbs / (2.0 * self.phi.sqrt())
        };
        let vov = vgs - (p * self.vto + self.gamma * body);
        let beta = self.kp * self.w / self.l;
        let i = if vov <= 0.0 {
            0.0
        } else if vds < vov {
            beta * (vov - 0.5 * vds) * vds * (1.0 + self.lambda * vds)
        } else {
            0.5 * beta * vov * vov * (1.0 + self.lambda * vds)
        };
        if swap {
            -p * i
        } else {
            p * i
        }
    }
}

/// Resistors, diodes, MOSFETs, grounded voltage sources and current sources (`(from, to, amps)`: `amps` leave
/// `from` and enter `to` through the source), with nodes named by their numbers.
#[derive(Clone, Default)]
struct Circuit {
    res: Vec<(usize, usize, f64)>,
    diodes: Vec<Diode>,
    mos: Vec<Mos>,
    vsrc: Vec<(usize, f64)>,
    isrc: Vec<(usize, usize, f64)>,
}

impl Circuit {
    fn deck(&self) -> String {
        let mut s = String::from("batch test\n");
        for (k, d) in self.diodes.iter().enumerate() {
            s += &format!(".model DM{k} D(IS={:e} N={})\n", d.is, d.n);
        }
        for (k, m) in self.mos.iter().enumerate() {
            s += &format!(
                ".model MM{k} {}(VTO={} KP={:e} LAMBDA={} GAMMA={} PHI={})\n",
                if m.pmos { "PMOS" } else { "NMOS" },
                m.vto,
                m.kp,
                m.lambda,
                m.gamma,
                m.phi
            );
        }
        for (k, &(node, v)) in self.vsrc.iter().enumerate() {
            s += &format!("V{} {node} 0 DC {v}\n", k + 1);
        }
        for (k, &(a, b, i)) in self.isrc.iter().enumerate() {
            s += &format!("I{} {a} {b} DC {i}\n", k + 1);
        }
        for (k, &(a, b, r)) in self.res.iter().enumerate() {
            s += &format!("R{} {a} {b} {r}\n", k + 1);
        }
        for (k, d) in self.diodes.iter().enumerate() {
            s += &format!("D{} {} {} DM{k} {}\n", k + 1, d.a, d.c, d.area);
        }
        for (k, m) in self.mos.iter().enumerate() {
            s += &format!(
                "M{} {} {} {} {} MM{k} W={:e} L={:e}\n",
                k + 1,
                m.d,
                m.g,
                m.s,
                m.b,
                m.w,
                m.l
            );
        }
        s + ".end\n"
    }

    /// Applies the overrides the way `set_device_parameter` defines them.
    fn with(&self, overrides: &[(String, String, f64)]) -> Circuit {
        let mut c = self.clone();
        for (dev, p, v) in overrides {
            let idx: usize = dev[1..].parse::<usize>().unwrap() - 1;
            match (dev.as_bytes()[0], p.as_str()) {
                (b'R', "r") => c.res[idx].2 = *v,
                (b'V', "dc") => c.vsrc[idx].1 = *v,
                (b'I', "dc") => c.isrc[idx].2 = *v,
                (b'D', "is") => {
                    // the effective saturation current, area included
                    c.diodes[idx].is = *v / c.diodes[idx].area;
                }
                (b'D', "n") => c.diodes[idx].n = *v,
                (b'D', "area") => c.diodes[idx].area = *v,
                (b'M', "w") => c.mos[idx].w = *v,
                (b'M', "l") => c.mos[idx].l = *v,
                (b'M', "vto") => c.mos[idx].vto = *v,
                (b'M', "kp") => c.mos[idx].kp = *v,
                (b'M', "lambda") => c.mos[idx].lambda = *v,
                _ => panic!("override {dev} {p}"),
            }
        }
        c
    }

    /// Largest KCL violation over the nodes not fixed by a voltage source, in units of `1e-9` of the currents
    /// that meet there plus `1e-12` A (the solver's `abstol`), and the largest source-voltage error.
    fn kcl_error(&self, v: &dyn Fn(usize) -> f64, nodes: usize) -> (f64, f64) {
        let mut sum = vec![0.0f64; nodes + 1];
        let mut mag = vec![0.0f64; nodes + 1];
        let mut add = |node: usize, i: f64| {
            sum[node] += i;
            mag[node] += i.abs();
        };
        for &(a, b, r) in &self.res {
            let i = (v(a) - v(b)) / r;
            add(a, i);
            add(b, -i);
        }
        for d in &self.diodes {
            let vd = v(d.a) - v(d.c);
            let is = d.is * d.area;
            let nvt = d.n * VT;
            let id = if vd >= -3.0 * nvt {
                is * ((vd / nvt).exp() - 1.0)
            } else {
                let arg = 3.0 * nvt / (vd * std::f64::consts::E);
                -is * (1.0 + arg * arg * arg)
            } + GMIN * vd;
            add(d.a, id);
            add(d.c, -id);
        }
        for m in &self.mos {
            let id = m.drain_current(v(m.d), v(m.g), v(m.s), v(m.b));
            add(m.d, id);
            add(m.s, -id);
            // bulk-drain and bulk-source junctions (anode at the bulk of an NMOS), each with `gmin` across it; none
            // where the terminal is the bulk itself
            for node in [m.d, m.s] {
                if node != m.b {
                    let (a, c) = if m.pmos { (node, m.b) } else { (m.b, node) };
                    let vj = v(a) - v(c);
                    let i = 1e-14 * ((vj / VT).exp() - 1.0) + GMIN * vj;
                    add(a, i);
                    add(c, -i);
                }
            }
        }
        for &(a, b, i) in &self.isrc {
            add(a, i);
            add(b, -i);
        }
        let fixed: Vec<usize> = self.vsrc.iter().map(|&(n, _)| n).collect();
        let mut worst = 0.0f64;
        for k in 1..=nodes {
            if !fixed.contains(&k) {
                worst = worst.max(sum[k].abs() / (1e-9 * mag[k] + 1e-12));
            }
        }
        let verr = self
            .vsrc
            .iter()
            .map(|&(n, val)| (v(n) - val).abs())
            .fold(0.0, f64::max);
        (worst, verr)
    }
}

/// The 32-diode cascade of benchmark b6.
fn cascade(n: usize) -> Circuit {
    let mut c = Circuit {
        vsrc: vec![(1, 5.0)],
        ..Default::default()
    };
    for k in 1..=n {
        let n_in = if k == 1 { 1 } else { n + k };
        let n_mid = n + k + 1;
        c.res.push((1, n_mid, 1000.0));
        c.diodes.push(Diode {
            a: n_mid,
            c: 0,
            is: 1e-14,
            n: 1.05,
            area: 1.0,
        });
        c.res.push((n_in, n_mid, 10000.0));
    }
    c
}

fn nmos(d: usize, g: usize, s: usize, b: usize) -> Mos {
    Mos {
        d,
        g,
        s,
        b,
        pmos: false,
        w: 2e-6,
        l: 1e-6,
        vto: 0.7,
        kp: 1.1e-4,
        lambda: 0.02,
        gamma: 0.4,
        phi: 0.7,
    }
}

fn pmos(d: usize, g: usize, s: usize, b: usize) -> Mos {
    Mos {
        pmos: true,
        w: 4e-6,
        vto: -0.8,
        kp: 4e-5,
        lambda: 0.03,
        gamma: 0.3,
        ..nmos(d, g, s, b)
    }
}

/// `n` CMOS inverters in a chain: VDD at node 1 (`V1`), the input at node 2 (`V2`), stage `k` drives node
/// `2 + k`. Every bulk junction lies across a drain and a source tied to the bulk (folded into the MOSFET).
fn inverter_chain(n: usize) -> Circuit {
    let mut c = Circuit {
        vsrc: vec![(1, 3.3), (2, 0.0)],
        ..Default::default()
    };
    for k in 1..=n {
        c.mos.push(nmos(2 + k, 1 + k, 0, 0));
        c.mos.push(pmos(2 + k, 1 + k, 1, 1));
    }
    c
}

/// Analog stages with body effect: a differential pair on a mirrored tail, a source follower, a PMOS common
/// source stage, an NMOS gain stage and a pass transistor that runs reversed (source above drain). VDD at node 1,
/// the inputs at nodes 2 and 3.
fn amplifier() -> Circuit {
    Circuit {
        vsrc: vec![(1, 3.3), (2, 1.6), (3, 1.5)],
        res: vec![
            (1, 4, 2e4),
            (1, 5, 2e4),
            (1, 7, 5e4),
            (8, 0, 2e4),
            (9, 0, 2e4),
            (1, 10, 2e4),
            (11, 0, 2e4),
        ],
        mos: vec![
            nmos(4, 2, 6, 0),
            nmos(5, 3, 6, 0),
            nmos(6, 7, 0, 0),
            nmos(7, 7, 0, 0),
            nmos(1, 4, 8, 0),
            pmos(9, 5, 1, 1),
            nmos(10, 9, 0, 0),
            nmos(11, 1, 10, 0),
        ],
        ..Default::default()
    }
}

struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
    fn pick(&mut self, n: usize) -> usize {
        ((self.next() * n as f64) as usize).min(n - 1)
    }
}

/// A random mesh of `nodes` nodes: every node grounded through a resistor, extra resistors and diodes (both
/// orientations, some between two internal nodes), two voltage sources and a current source.
fn mesh(nodes: usize, seed: u64) -> Circuit {
    let mut g = Lcg(seed);
    let mut c = Circuit {
        vsrc: vec![(1, 3.0), (2, -1.5)],
        isrc: vec![(0, nodes, 2e-3)],
        ..Default::default()
    };
    for k in 1..=nodes {
        c.res.push((k, 0, 1e3 + 9e3 * g.next()));
    }
    for _ in 0..2 * nodes {
        let a = 1 + g.pick(nodes);
        let b = 1 + g.pick(nodes);
        if a != b {
            c.res.push((a, b, 100.0 + 5e3 * g.next()));
        }
    }
    for _ in 0..nodes {
        let a = 1 + g.pick(nodes);
        let b = if g.next() < 0.3 { 0 } else { 1 + g.pick(nodes) };
        if a != b {
            c.diodes.push(Diode {
                a,
                c: b,
                is: 1e-15 * (1.0 + 99.0 * g.next()),
                n: 1.0 + 0.5 * g.next(),
                area: 1.0 + g.next(),
            });
        }
    }
    c
}

fn nodes_of(c: &Circuit) -> usize {
    let mut m = 0;
    for &(a, b, _) in &c.res {
        m = m.max(a).max(b);
    }
    for d in &c.diodes {
        m = m.max(d.a).max(d.c);
    }
    for t in &c.mos {
        m = m.max(t.d).max(t.g).max(t.s).max(t.b);
    }
    m
}

fn assert_close(what: &str, a: &[f64], b: &[f64], tol: f64) -> f64 {
    assert_eq!(a.len(), b.len());
    let mut worst = 0.0f64;
    for (i, (&x, &y)) in a.iter().zip(b).enumerate() {
        let e = (x - y).abs() / (1.0 + x.abs().max(y.abs()));
        assert!(
            e <= tol,
            "{what}: unknown {i}: {x} vs {y} (relative {e:e} > {tol:e})"
        );
        worst = worst.max(e);
    }
    worst
}

fn check_kcl(sim: &Simulator, c: &Circuit, x: &[f64]) {
    let nodes = nodes_of(c);
    let v = |k: usize| sim.node_voltage(x, &k.to_string()).unwrap();
    let (kcl, verr) = c.kcl_error(&v, nodes);
    assert!(kcl <= 1.0, "KCL violated: {kcl:e} x (1e-9 |I| + 1 pA)");
    assert!(verr <= 1e-12, "source voltage off by {verr:e} V");
}

fn supply_corners(n: usize) -> Vec<BatchCircuitInput> {
    (0..n)
        .map(|k| BatchCircuitInput {
            overrides: vec![(
                "V1".to_string(),
                "dc".to_string(),
                -1.0 + 6.0 * k as f64 / (n - 1) as f64,
            )],
        })
        .collect()
}

fn mesh_corners(c: &Circuit, n: usize, seed: u64) -> Vec<BatchCircuitInput> {
    let mut g = Lcg(seed);
    (0..n)
        .map(|_| {
            let mut o = vec![
                ("V1".to_string(), "dc".to_string(), 0.5 + 4.0 * g.next()),
                ("V2".to_string(), "dc".to_string(), -2.0 * g.next()),
                ("I1".to_string(), "dc".to_string(), 5e-3 * (g.next() - 0.5)),
            ];
            for _ in 0..3 {
                let k = 1 + g.pick(c.res.len());
                o.push((format!("R{k}"), "r".to_string(), 50.0 + 2e4 * g.next()));
            }
            for _ in 0..3 {
                let k = 1 + g.pick(c.diodes.len());
                let (p, v) = match g.pick(3) {
                    0 => ("is", 1e-16 * (1.0 + 1e3 * g.next())),
                    1 => ("n", 1.0 + g.next()),
                    _ => ("area", 0.5 + 2.0 * g.next()),
                };
                o.push((format!("D{k}"), p.to_string(), v));
            }
            BatchCircuitInput { overrides: o }
        })
        .collect()
}

fn chain_corners(n: usize) -> Vec<BatchCircuitInput> {
    let mut g = Lcg(5);
    (0..n)
        .map(|k| {
            let vdd = 1.8 + 1.5 * g.next();
            BatchCircuitInput {
                overrides: vec![
                    ("V1".to_string(), "dc".to_string(), vdd),
                    (
                        "V2".to_string(),
                        "dc".to_string(),
                        vdd * k as f64 / (n - 1) as f64,
                    ),
                ],
            }
        })
        .collect()
}

/// Supply and input corners with every MOSFET parameter kind varied on random devices (W, L, VTO, KP, LAMBDA,
/// some overridden twice in one corner) and a resistor.
fn amplifier_corners(c: &Circuit, n: usize, seed: u64) -> Vec<BatchCircuitInput> {
    let mut g = Lcg(seed);
    (0..n)
        .map(|_| {
            let mut o = vec![
                ("V1".to_string(), "dc".to_string(), 2.5 + 1.1 * g.next()),
                ("V2".to_string(), "dc".to_string(), 1.2 + 0.6 * g.next()),
                ("V3".to_string(), "dc".to_string(), 1.2 + 0.6 * g.next()),
            ];
            for _ in 0..5 {
                let k = g.pick(c.mos.len());
                let m = c.mos[k];
                let s = 0.8 + 0.4 * g.next();
                let (p, v) = match g.pick(5) {
                    0 => ("w", m.w * s),
                    1 => ("l", m.l * s),
                    2 => ("vto", m.vto * s),
                    3 => ("kp", m.kp * s),
                    _ => ("lambda", m.lambda * s),
                };
                o.push((format!("M{}", k + 1), p.to_string(), v));
            }
            let k = 1 + g.pick(c.res.len());
            o.push((format!("R{k}"), "r".to_string(), 1e4 + 3e4 * g.next()));
            BatchCircuitInput { overrides: o }
        })
        .collect()
}

/// The kernel path (`cpu_ref`) against the CPU batch API (within the Newton tolerance) and the KCL check; returns
/// the number of CPU fallbacks.
fn check_against_cpu(name: &str, c: &Circuit, inputs: &[BatchCircuitInput]) -> usize {
    let sim = Simulator::from_spice(&c.deck()).unwrap();
    let cpu = sim.run_batch_dc_op(inputs).unwrap();
    let run = sim
        .run_batch_dc_op_with(inputs, BatchDcEngine::CpuRef)
        .unwrap();
    assert_eq!(run.unsupported, None);
    eprintln!(
        "{name} plan: {}",
        sim.batch_dc_plan(&inputs[..1]).unwrap().0.summary()
    );
    let mut worst = 0.0f64;
    for (k, (a, b)) in run.outputs.iter().zip(&cpu).enumerate() {
        worst = worst.max(assert_close(
            &format!("{name} corner {k}"),
            &a.solution,
            &b.solution,
            1e-9,
        ));
        check_kcl(&sim, &c.with(&inputs[k].overrides), &a.solution);
    }
    eprintln!(
        "{name}: {} corners, {} CPU fallbacks, largest relative difference to the CPU batch API {worst:e}",
        inputs.len(),
        run.cpu_fallback
    );
    run.cpu_fallback
}

#[test]
fn cpu_ref_matches_the_cpu_batch_api_on_a_cmos_inverter_chain() {
    let inputs = chain_corners(256);
    let fallback = check_against_cpu("inverter chain", &inverter_chain(20), &inputs);
    assert!(fallback <= inputs.len() / 10, "{fallback} CPU fallbacks");
}

#[test]
fn cpu_ref_maps_every_mosfet_parameter_like_set_device_parameter() {
    let c = amplifier();
    let inputs = amplifier_corners(&c, 192, 17);
    let fallback = check_against_cpu("amplifier", &c, &inputs);
    assert!(fallback <= inputs.len() / 10, "{fallback} CPU fallbacks");
}

#[test]
fn cpu_ref_matches_the_cpu_batch_api_on_the_b6_cascade() {
    let c = cascade(32);
    let sim = Simulator::from_spice(&c.deck()).unwrap();
    let inputs = supply_corners(256);
    let cpu = sim.run_batch_dc_op(&inputs).unwrap();
    let run = sim
        .run_batch_dc_op_with(&inputs, BatchDcEngine::CpuRef)
        .unwrap();
    assert_eq!(run.unsupported, None);
    assert_eq!(
        run.cpu_fallback, 0,
        "every supply corner converges in the kernel"
    );
    eprintln!(
        "cascade plan: {}",
        sim.batch_dc_plan(&inputs[..1]).unwrap().0.summary()
    );
    let mut worst = 0.0f64;
    for (k, (a, b)) in run.outputs.iter().zip(&cpu).enumerate() {
        worst = worst.max(assert_close(
            &format!("corner {k}"),
            &a.solution,
            &b.solution,
            1e-9,
        ));
        check_kcl(&sim, &c.with(&inputs[k].overrides), &a.solution);
    }
    eprintln!("cascade: largest relative difference to the CPU batch API {worst:e}");
}

#[test]
fn cpu_ref_maps_every_parameter_kind_like_set_device_parameter() {
    let c = mesh(40, 7);
    let sim = Simulator::from_spice(&c.deck()).unwrap();
    let inputs = mesh_corners(&c, 96, 11);
    let cpu = sim.run_batch_dc_op(&inputs).unwrap();
    let run = sim
        .run_batch_dc_op_with(&inputs, BatchDcEngine::CpuRef)
        .unwrap();
    assert_eq!(run.unsupported, None);
    assert!(
        run.cpu_fallback <= inputs.len() / 10,
        "{} fallbacks",
        run.cpu_fallback
    );
    let mut worst = 0.0f64;
    for (k, (a, b)) in run.outputs.iter().zip(&cpu).enumerate() {
        worst = worst.max(assert_close(
            &format!("corner {k}"),
            &a.solution,
            &b.solution,
            1e-9,
        ));
        check_kcl(&sim, &c.with(&inputs[k].overrides), &a.solution);
    }
    eprintln!(
        "mesh: {} CPU fallbacks, largest relative difference {worst:e}",
        run.cpu_fallback
    );
}

#[test]
fn unsupported_circuits_run_on_the_cpu_solver() {
    // a diode with breakdown is a full junction
    let deck = "clamp\n.model DZ D(IS=1e-14 BV=2.5)\nV1 in 0 DC 1\nR1 in out 1k\nD1 0 out DZ\n\
                R2 out 0 10k\n.end\n";
    let sim = Simulator::from_spice(deck).unwrap();
    let inputs: Vec<BatchCircuitInput> = (0..8)
        .map(|k| BatchCircuitInput {
            overrides: vec![("V1".into(), "dc".into(), 0.5 * k as f64)],
        })
        .collect();
    let run = sim
        .run_batch_dc_op_with(&inputs, BatchDcEngine::CpuRef)
        .unwrap();
    assert!(run
        .unsupported
        .as_deref()
        .is_some_and(|r| r.contains("breakdown")));
    let cpu = sim.run_batch_dc_op(&inputs).unwrap();
    for (a, b) in run.outputs.iter().zip(&cpu) {
        assert_eq!(a.solution, b.solution);
    }
}

#[test]
fn unconverged_corners_go_to_the_cpu_solver() {
    let c = cascade(8);
    let mut sim = Simulator::from_spice(&c.deck()).unwrap();
    // too few iterations for the kernel: every corner is re-solved, with the CPU's homotopy fallbacks
    sim.nonlinear_solver.options.max_iters = 3;
    let inputs = supply_corners(16);
    let (plan, params) = sim.batch_dc_plan(&inputs).unwrap();
    let raw = solve_cpu_ref(&plan, &params, inputs.len());
    let failed = raw.unconverged().count();
    assert!(failed > 0, "3 iterations should not converge from x = 0");
    assert!(raw
        .status
        .iter()
        .all(|&s| s == status::CONVERGED || s == status::NO_CONVERGENCE));
    let run = sim.run_batch_dc_op_with(&inputs, BatchDcEngine::CpuRef);
    let cpu = sim.run_batch_dc_op(&inputs);
    match (run, cpu) {
        (Ok(run), Ok(cpu)) => {
            assert_eq!(run.cpu_fallback, failed);
            for (k, (a, b)) in run.outputs.iter().zip(&cpu).enumerate() {
                assert_close(&format!("corner {k}"), &a.solution, &b.solution, 1e-9);
            }
        }
        (Err(_), Err(_)) => {}
        (a, b) => panic!("kernel path {:?} vs CPU {:?}", a.is_ok(), b.is_ok()),
    }
}

/// Rule 5: the CUDA kernel against `cpu_ref`, with an explicit tolerance. Skipped without a CUDA driver unless
/// `RUSTSPICE_REQUIRE_GPU=1`.
#[test]
fn cuda_kernel_matches_cpu_ref() {
    let gpu = match CudaBatchSolver::new(0) {
        Ok(g) => g,
        Err(e) => {
            assert!(
                std::env::var("RUSTSPICE_REQUIRE_GPU").as_deref() != Ok("1"),
                "RUSTSPICE_REQUIRE_GPU=1 but no GPU: {e}"
            );
            eprintln!("skipped: {e}");
            return;
        }
    };
    eprintln!("GPU: {}", gpu.device_name());
    for (name, c, inputs) in [
        ("cascade", cascade(32), supply_corners(2000)),
        ("mesh", mesh(40, 7), mesh_corners(&mesh(40, 7), 1000, 13)),
        ("inverter chain", inverter_chain(20), chain_corners(1000)),
        (
            "amplifier",
            amplifier(),
            amplifier_corners(&amplifier(), 1000, 19),
        ),
    ] {
        let sim = Simulator::from_spice(&c.deck()).unwrap();
        let (plan, params) = sim.batch_dc_plan(&inputs).unwrap();
        let n = inputs.len();
        let reference = solve_cpu_ref(&plan, &params, n);
        let whole = gpu.solve(&plan, &params, n, None).unwrap();
        // a budget of ~300 corners per launch exercises the chunking
        let budget = 300 * 8 * (plan.workspace_slots() + plan.num_params() + 1);
        let chunked = gpu.solve(&plan, &params, n, Some(budget)).unwrap();
        assert_eq!(whole.status, chunked.status);
        assert_eq!(whole.x, chunked.x, "chunking must not change any bit");
        let mut worst = 0.0f64;
        let mut differ = 0;
        for (k, input) in inputs.iter().enumerate() {
            if whole.status[k] != reference.status[k] {
                differ += 1;
                continue;
            }
            if whole.status[k] == status::CONVERGED {
                // libdevice and libm round exp / ln differently in the last bit; Newton absorbs that
                worst = worst.max(assert_close(
                    &format!("{name} corner {k}"),
                    whole.corner(k),
                    reference.corner(k),
                    1e-12,
                ));
                check_kcl(&sim, &c.with(&input.overrides), whole.corner(k));
            }
        }
        assert!(
            differ <= n / 100,
            "{name}: {differ} corners end with another status"
        );
        let bit_equal = (0..n)
            .filter(|&k| whole.corner(k) == reference.corner(k))
            .count();
        eprintln!(
            "{name}: {n} corners, {differ} status differences, {bit_equal} bit-identical, largest relative difference {worst:e}"
        );
    }
}
