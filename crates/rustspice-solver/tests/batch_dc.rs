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

/// Resistors, diodes, grounded voltage sources and current sources (`(from, to, amps)`: `amps` leave `from` and
/// enter `to` through the source), with nodes named by their numbers.
#[derive(Clone, Default)]
struct Circuit {
    res: Vec<(usize, usize, f64)>,
    diodes: Vec<Diode>,
    vsrc: Vec<(usize, f64)>,
    isrc: Vec<(usize, usize, f64)>,
}

impl Circuit {
    fn deck(&self) -> String {
        let mut s = String::from("batch test\n");
        for (k, d) in self.diodes.iter().enumerate() {
            s += &format!(".model DM{k} D(IS={:e} N={})\n", d.is, d.n);
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
    let deck = "inverter\n.model NM NMOS(VTO=0.7 KP=1e-4)\n.model PM PMOS(VTO=-0.7 KP=5e-5)\n\
                VDD vdd 0 DC 3\nVIN in 0 DC 1\nM1 out in 0 0 NM W=2u L=1u\nM2 out in vdd vdd PM W=4u L=1u\n\
                R1 out 0 1meg\n.end\n";
    let sim = Simulator::from_spice(deck).unwrap();
    let inputs: Vec<BatchCircuitInput> = (0..8)
        .map(|k| BatchCircuitInput {
            overrides: vec![("VIN".into(), "dc".into(), 0.4 * k as f64)],
        })
        .collect();
    let run = sim
        .run_batch_dc_op_with(&inputs, BatchDcEngine::CpuRef)
        .unwrap();
    assert!(run
        .unsupported
        .as_deref()
        .is_some_and(|r| r.contains("MOSFET")));
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
