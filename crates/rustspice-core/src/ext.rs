//! Devices every backend evaluates in `f64` on the host after its own device kernels
//! ([`DeviceSoaBuffer::evaluate_ext_devices`]): inductor couplings (`K`), polynomial controlled sources
//! (`E`/`G ... POLY(1)`), PN junctions with the full diode equation and charge storage (diodes with series
//! resistance, breakdown, high injection, recombination current or capacitance; MOSFET bulk junctions) and the
//! Meyer gate capacitances of level-1 MOSFETs.
//!
//! A charge `q(v)` is integrated with the companion coefficients of the step: `i = a0 q(v) + hist`, where the
//! solver sets `hist` before every step ([`ExtDevices::set_charge_history`]) and commits the accepted charges
//! ([`ExtDevices::commit_charges`]).

use serde::{Deserialize, Serialize};

use crate::error::{SimError, SimResult};
use crate::soa::DeviceSoaBuffer;

/// Largest Shockley exponent argument `v / (n Vt)` evaluated with the exact exponential; above it a `C^1` linear
/// continuation keeps the Jacobian finite and the solver rejects a converged point that still lies there.
pub const DIODE_EXP_ARG_MAX: f64 = 600.0;
const DIODE_EXP_SLOPE_MAX: f64 = 3.773_020_300_929_939_7e260; // 600.0_f64.exp()

/// `exp(arg)` and its derivative, continued linearly above [`DIODE_EXP_ARG_MAX`]; the flag reports the
/// continuation.
#[inline]
pub fn diode_exp(arg: f64) -> (f64, f64, bool) {
    if arg <= DIODE_EXP_ARG_MAX {
        let e = arg.exp();
        (e, e, false)
    } else {
        let excess = arg - DIODE_EXP_ARG_MAX;
        (
            DIODE_EXP_SLOPE_MAX * (1.0 + excess),
            DIODE_EXP_SLOPE_MAX,
            true,
        )
    }
}

/// SPICE PN-junction logarithmic voltage step limiter (`pnjlim`): a forward step beyond the critical voltage is
/// compressed logarithmically so the exponential cannot overflow between Newton iterations.
#[inline]
pub fn pnjlim(vnew: f64, vold: f64, vt: f64, vcrit: f64) -> f64 {
    if vnew > vcrit && (vnew - vold).abs() > (vt + vt) {
        if vold > 0.0 {
            let arg = 1.0 + (vnew - vold) / vt;
            if arg > 0.0 {
                vold + vt * arg.ln()
            } else {
                vcrit
            }
        } else {
            vt * (vnew / vt).ln()
        }
    } else {
        vnew
    }
}

/// History of one charge-storing branch: `i = a0 q + hist`.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ChargeState {
    pub q_prev: f64,
    pub q_prev2: f64,
    pub q_prev3: f64,
    pub i_prev: f64,
    /// `a1 q_n + a2 q_(n-1)` (minus `i_n` for the trapezoid) of the step being solved.
    pub hist: f64,
}

impl ChargeState {
    fn start(&mut self, q: f64) {
        *self = Self {
            q_prev: q,
            q_prev2: q,
            q_prev3: q,
            ..Self::default()
        };
    }

    fn set_hist(&mut self, a1: f64, a2: f64, uses_rate: bool) {
        self.hist = a1 * self.q_prev + a2 * self.q_prev2;
        if uses_rate {
            self.hist -= self.i_prev;
        }
    }

    fn commit(&mut self, q: f64, a0: f64) {
        self.i_prev = a0 * q + self.hist;
        self.q_prev3 = self.q_prev2;
        self.q_prev2 = self.q_prev;
        self.q_prev = q;
    }
}

/// Coupling of two inductors: the entries `-M a0` between their branch equations.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Mutual {
    pub name: String,
    /// Indices of the two inductors in the inductor arrays.
    pub ind_a: usize,
    pub ind_b: usize,
    pub m: f64,
    pub cond_buf_idx: u32,
}

/// Polynomial `f(x) = sum_k coeffs[k] x^k` of the control voltage `x = v(nc_pos) - v(nc_neg)`, linearised as
/// `f'(x0) x + (f(x0) - f'(x0) x0)`; the compiler stamps the slope and the offset as a current or as a
/// branch-equation term.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PolySource {
    pub name: String,
    pub nc_pos: usize,
    pub nc_neg: usize,
    pub coeffs: Vec<f64>,
    pub cond_buf_idx: u32,
    pub rhs_buf_idx: u32,
}

impl PolySource {
    /// `(f(x), f'(x))` by Horner's rule.
    #[inline]
    pub fn eval(&self, x: f64) -> (f64, f64) {
        let mut f = 0.0;
        let mut df = 0.0;
        for &c in self.coeffs.iter().rev() {
            df = df * x + f;
            f = f * x + c;
        }
        (f, df)
    }
}

/// Depletion charge and capacitance of a junction `cj0 / (1 - v/vj)^m`, continued for `v >= fc vj` by the
/// tangent line of the capacitance.
#[inline]
pub fn depletion_charge(v: f64, cj0: f64, vj: f64, m: f64, fc: f64) -> (f64, f64) {
    if cj0 == 0.0 {
        return (0.0, 0.0);
    }
    let one_m = 1.0 - m;
    if v < fc * vj {
        let arg = 1.0 - v / vj;
        let sarg = (-m * arg.ln()).exp();
        let q = if one_m.abs() < 1e-9 {
            -cj0 * vj * arg.ln()
        } else {
            cj0 * vj * (1.0 - arg * sarg) / one_m
        };
        (q, cj0 * sarg)
    } else {
        let f1 = if one_m.abs() < 1e-9 {
            -vj * (1.0 - fc).ln()
        } else {
            vj * (1.0 - (1.0 - fc).powf(one_m)) / one_m
        };
        let f2 = (1.0 - fc).powf(1.0 + m);
        let f3 = 1.0 - fc * (1.0 + m);
        let v0 = fc * vj;
        let q = cj0 * f1 + cj0 / f2 * (f3 * (v - v0) + m / (2.0 * vj) * (v * v - v0 * v0));
        (q, cj0 / f2 * (f3 + m * v / vj))
    }
}

/// PN junction between `anode` and `cathode` (1-based node ids, `0` = ground): the SPICE diode equation with
/// breakdown, high-injection knee and recombination current, depletion (bottom and sidewall) and diffusion
/// charge. Parameters are at the device temperature and scaled by area.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Junction {
    pub name: String,
    pub anode: usize,
    pub cathode: usize,
    pub is: f64,
    pub n: f64,
    /// Breakdown voltage fitted so that the current at `-BV` is `IBV` (`INFINITY`: no breakdown).
    pub xbv: f64,
    /// High-injection knee current (`INFINITY`: none).
    pub ikf: f64,
    /// Recombination saturation current (0: none) and its emission coefficient.
    pub isr: f64,
    pub nr: f64,
    pub cj0: f64,
    pub vj: f64,
    pub m: f64,
    pub fc: f64,
    pub cjsw: f64,
    pub mjsw: f64,
    pub tt: f64,
    /// Reverse current below `-3 n Vt` from the cubic `-is (1 + (3 n Vt / (e v))^3)` (diode elements); `false`:
    /// the exponential throughout (MOSFET bulk junctions).
    pub cubic_reverse: bool,
    /// Junction voltage of the last evaluation (the reference of the step limiter).
    pub vd: f64,
    /// `n Vt` and the critical voltage of the step limiter, `n Vt ln(n Vt / (sqrt(2) is))` (set by the compiler).
    pub nvt: f64,
    pub vcrit: f64,
    /// MOSFET whose drain-source slots this junction shares (its other terminal is the bulk): the junction adds
    /// its conductance to the `gds` slot and its current, oriented drain to source, to the current slot that the
    /// MOSFET kernel wrote just before.
    pub fold_mos: Option<usize>,
    /// `+1` / `-1`: the junction current (anode to cathode) flows drain to source / source to drain.
    pub fold_sign: f64,
    pub charge: ChargeState,
    pub cond_buf_idx: u32,
    pub rhs_buf_idx: u32,
}

impl Junction {
    #[inline]
    pub fn stores_charge(&self) -> bool {
        self.cj0 > 0.0 || self.cjsw > 0.0 || self.tt > 0.0
    }

    /// Breakdown voltage at which the reverse current equals `ibv`: the exponential `is exp(-(xbv + v) / (n vt))`
    /// must reach `ibv` at `v = -bv`. `None`: no breakdown.
    pub fn fit_breakdown(bv: Option<f64>, ibv: f64, is: f64, n: f64, vt: f64) -> f64 {
        match bv {
            Some(bv) if bv > 0.0 => {
                let nvt = n * vt;
                if ibv > is {
                    bv - nvt * (ibv / is).ln()
                } else {
                    bv
                }
            }
            _ => f64::INFINITY,
        }
    }

    /// Junction current and its derivative at `vd` (without `gmin`), plus the diffusion part `is (e - 1)`
    /// whose charge is `tt` times it.
    #[inline]
    pub fn current(&self, vd: f64, vt: f64) -> (f64, f64, f64, f64) {
        let nvt = self.n * vt;
        let (mut id, mut gd) = if vd >= -3.0 * nvt || !self.cubic_reverse {
            let arg = vd / nvt;
            if arg < -40.0 {
                // e^-40 is below half an ulp of 1: `is (e - 1)` rounds to `-is`
                (-self.is, 0.0)
            } else {
                let (e, de, _) = diode_exp(arg);
                (self.is * (e - 1.0), self.is * de / nvt)
            }
        } else if vd >= -self.xbv {
            let a = 3.0 * nvt / (vd * std::f64::consts::E);
            let a3 = a * a * a;
            (-self.is * (1.0 + a3), self.is * 3.0 * a3 / vd)
        } else {
            let e = (-(self.xbv + vd) / nvt).exp();
            (-self.is * e, self.is * e / nvt)
        };
        if self.ikf.is_finite() && id > 0.0 {
            // id / (1 + sqrt(id / ikf))
            let s = (id / self.ikf).sqrt();
            let den = 1.0 + s;
            let did = (1.0 + 0.5 * s) / (den * den);
            id /= den;
            gd *= did;
        }
        let (diff, gdiff) = (id, gd);
        if self.isr > 0.0 {
            let nrvt = self.nr * vt;
            let (e, de, _) = diode_exp(vd / nrvt);
            let u = 1.0 - vd / self.vj;
            let base = u * u + 0.005;
            let kgen = base.powf(0.5 * self.m);
            let dkgen = 0.5 * self.m * base.powf(0.5 * self.m - 1.0) * 2.0 * u * (-1.0 / self.vj);
            let irec = self.isr * (e - 1.0);
            id += irec * kgen;
            gd += self.isr * de / nrvt * kgen + irec * dkgen;
        }
        (id, gd, diff, gdiff)
    }

    /// Stored charge and capacitance at `vd`.
    #[inline]
    pub fn charge_at(&self, vd: f64, vt: f64) -> (f64, f64) {
        let (mut q, mut c) = depletion_charge(vd, self.cj0, self.vj, self.m, self.fc);
        if self.cjsw > 0.0 {
            let (qs, cs) = depletion_charge(vd, self.cjsw, self.vj, self.mjsw, self.fc);
            q += qs;
            c += cs;
        }
        if self.tt > 0.0 {
            let (_, _, diff, gdiff) = self.current(vd, vt);
            q += self.tt * diff;
            c += self.tt * gdiff;
        }
        (q, c)
    }

    /// Sets the constants of the evaluation at the thermal voltage `vt`.
    pub fn prepare(&mut self, vt: f64) {
        self.nvt = self.n * vt;
        self.vcrit = critical_voltage(self.nvt, self.is);
    }

    /// Whether the junction is a plain exponential without charge (a [`BulkJunctions`] entry).
    pub fn is_plain(&self) -> bool {
        !self.cubic_reverse
            && self.xbv == f64::INFINITY
            && self.ikf == f64::INFINITY
            && self.isr == 0.0
            && !self.stores_charge()
    }

    #[inline]
    fn voltage(&self, x: &[f64]) -> f64 {
        DeviceSoaBuffer::node_voltage(x, self.anode)
            - DeviceSoaBuffer::node_voltage(x, self.cathode)
    }
}

/// Critical voltage of the junction step limiter, `n Vt ln(n Vt / (sqrt(2) is))`.
#[inline]
pub fn critical_voltage(nvt: f64, is: f64) -> f64 {
    nvt * (nvt / (std::f64::consts::SQRT_2 * is)).ln()
}

/// Plain exponential junctions `is (exp(v / (n Vt)) - 1)` without charge (MOSFET bulk junctions without
/// capacitance), in arrays of their own: every MOSFET has one or two, so they are evaluated in one tight loop.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct BulkJunctions {
    pub names: Vec<String>,
    pub anode: Vec<usize>,
    pub cathode: Vec<usize>,
    pub is: Vec<f64>,
    pub nvt: Vec<f64>,
    /// `1 / (n Vt)` and `is / (n Vt)`.
    pub inv_nvt: Vec<f64>,
    pub g_sat: Vec<f64>,
    pub vcrit: Vec<f64>,
    /// Junction voltage of the last evaluation (the reference of the step limiter).
    pub vd: Vec<f64>,
    pub cond_buf_idx: Vec<u32>,
    pub rhs_buf_idx: Vec<u32>,
    /// `0`: own slots; `+1` / `-1`: shares the drain-source slots of a MOSFET (see [`Junction::fold_sign`]).
    pub fold_sign: Vec<f64>,
}

impl BulkJunctions {
    #[inline]
    pub fn len(&self) -> usize {
        self.anode.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.anode.is_empty()
    }

    /// Adds the plain junction `j` (its slots and `nvt` are set by the compiler).
    pub fn push(&mut self, j: &Junction) {
        self.names.push(j.name.clone());
        self.anode.push(j.anode);
        self.cathode.push(j.cathode);
        self.is.push(j.is);
        self.nvt.push(j.nvt);
        self.inv_nvt.push(1.0 / j.nvt);
        self.g_sat.push(j.is / j.nvt);
        self.vcrit.push(j.vcrit);
        self.vd.push(0.0);
        self.cond_buf_idx.push(j.cond_buf_idx);
        self.rhs_buf_idx.push(j.rhs_buf_idx);
        self.fold_sign.push(if j.fold_mos.is_some() {
            j.fold_sign
        } else {
            0.0
        });
    }

    #[inline]
    fn voltage(&self, k: usize, x: &[f64]) -> f64 {
        DeviceSoaBuffer::node_voltage(x, self.anode[k])
            - DeviceSoaBuffer::node_voltage(x, self.cathode[k])
    }

    /// Evaluates every junction into `cond` / `rhs`; returns whether the step limiter acted.
    fn evaluate(&mut self, x: &[f64], gmin: f64, cond: &mut [f64], rhs: &mut [f64]) -> bool {
        let mut limited = false;
        let n = self.len();
        let (anode, cathode) = (&self.anode[..n], &self.cathode[..n]);
        let (is, nvt, vcrit) = (&self.is[..n], &self.nvt[..n], &self.vcrit[..n]);
        let (inv_nvt, g_sat) = (&self.inv_nvt[..n], &self.g_sat[..n]);
        let (ci, ri, sign) = (
            &self.cond_buf_idx[..n],
            &self.rhs_buf_idx[..n],
            &self.fold_sign[..n],
        );
        for (k, vd_ref) in self.vd[..n].iter_mut().enumerate() {
            let raw = DeviceSoaBuffer::node_voltage(x, anode[k])
                - DeviceSoaBuffer::node_voltage(x, cathode[k]);
            let nvt_k = nvt[k];
            let vd = if raw > vcrit[k] {
                let v = pnjlim(raw, *vd_ref, nvt_k, vcrit[k]);
                limited |= v != raw || v > DIODE_EXP_ARG_MAX * nvt_k;
                v
            } else {
                raw
            };
            *vd_ref = vd;
            let (g, ieq) = if vd < -40.0 * nvt_k {
                // e^-40 is below half an ulp of 1: the current is `-is + gmin vd`
                (gmin, -is[k])
            } else {
                let arg = vd * inv_nvt[k];
                let (em1, e) = if arg.abs() < 1e-3 {
                    // near zero bias (most junctions of a settled circuit): Taylor series of e^x - 1 (truncation
                    // below 2e-21), cheaper and free of the cancellation of exp(x) - 1
                    let em1 = arg
                        * (1.0
                            + arg
                                * (0.5
                                    + arg
                                        * (1.0 / 6.0 + arg * (1.0 / 24.0 + arg * (1.0 / 120.0)))));
                    (em1, 1.0 + em1)
                } else {
                    let (e, de, _) = diode_exp(arg);
                    (e - 1.0, de)
                };
                // (plain products: without a target FMA, `mul_add` is a library call)
                let g = g_sat[k] * e + gmin;
                (g, is[k] * em1 + gmin * vd - g * vd)
            };
            let (c, r) = (ci[k] as usize, ri[k] as usize);
            if sign[k] == 0.0 {
                cond[c] = g;
                rhs[r] = ieq;
            } else {
                cond[c] += g;
                rhs[r] += sign[k] * ieq;
            }
        }
        limited
    }
}

/// Meyer gate capacitances of a level-1 MOSFET between the gate and the (internal) source, drain and bulk.
/// Each is integrated as the charge `q_n + (C(v_n) + C(v)) / 2 (v - v_n)` from the last accepted point.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MeyerCaps {
    pub name: String,
    pub drain: usize,
    pub gate: usize,
    pub source: usize,
    pub bulk: usize,
    pub polarity: f64,
    /// `polarity * VTO` at the device temperature, `GAMMA`, `PHI`.
    pub vto: f64,
    pub gamma: f64,
    pub phi: f64,
    /// `Cox W Leff` (F).
    pub cox: f64,
    /// Gate-source, gate-drain and gate-bulk voltages and capacitances at the last accepted point.
    pub v_prev: [f64; 3],
    pub c_prev: [f64; 3],
    pub charge: [ChargeState; 3],
    pub cond_buf_idx: [u32; 3],
    pub rhs_buf_idx: [u32; 3],
}

impl MeyerCaps {
    /// The three branch voltages `v(g) - v(s|d|b)`.
    #[inline]
    fn voltages(&self, x: &[f64]) -> [f64; 3] {
        let vg = DeviceSoaBuffer::node_voltage(x, self.gate);
        [
            vg - DeviceSoaBuffer::node_voltage(x, self.source),
            vg - DeviceSoaBuffer::node_voltage(x, self.drain),
            vg - DeviceSoaBuffer::node_voltage(x, self.bulk),
        ]
    }

    /// Meyer capacitances `(Cgs, Cgd, Cgb)` at the node voltages `x`.
    pub fn capacitances(&self, x: &[f64]) -> [f64; 3] {
        let p = self.polarity;
        let vd = DeviceSoaBuffer::node_voltage(x, self.drain);
        let vg = DeviceSoaBuffer::node_voltage(x, self.gate);
        let vs = DeviceSoaBuffer::node_voltage(x, self.source);
        let vb = DeviceSoaBuffer::node_voltage(x, self.bulk);
        // the source is the lower terminal in polarised coordinates
        let reversed = p * (vd - vs) < 0.0;
        let (vgs, vgd, vbs) = if reversed {
            (p * (vg - vd), p * (vg - vs), p * (vb - vd))
        } else {
            (p * (vg - vs), p * (vg - vd), p * (vb - vs))
        };
        let phi = self.phi;
        let sqrt_phi = phi.sqrt();
        let von = if self.gamma > 0.0 {
            if vbs <= 0.0 {
                self.vto + self.gamma * ((phi - vbs).sqrt() - sqrt_phi)
            } else {
                self.vto - self.gamma * vbs / (2.0 * sqrt_phi)
            }
        } else {
            self.vto
        };
        let (cgs, cgd, cgb) = meyer(vgs, vgd, von, phi, self.cox);
        if reversed {
            [cgd, cgs, cgb]
        } else {
            [cgs, cgd, cgb]
        }
    }
}

/// Meyer's piecewise gate capacitances `(Cgs, Cgd, Cgb)` of a level-1 channel (`vgs >= vgd`, polarised).
#[inline]
pub fn meyer(vgs: f64, vgd: f64, von: f64, phi: f64, cox: f64) -> (f64, f64, f64) {
    let vgst = vgs - von;
    let two_thirds = 2.0 / 3.0 * cox;
    if vgst <= -phi {
        (0.0, 0.0, cox)
    } else if vgst <= -0.5 * phi {
        (0.0, 0.0, -vgst * cox / phi)
    } else if vgst <= 0.0 {
        (
            4.0 / 3.0 * vgst * cox / phi + two_thirds,
            0.0,
            -vgst * cox / phi,
        )
    } else {
        let vds = vgs - vgd;
        let vdsat = vgst;
        if vds >= vdsat {
            (two_thirds, 0.0, 0.0)
        } else {
            let d = 2.0 * vdsat - vds;
            let d1 = vdsat - vds;
            let d2 = d * d;
            (
                two_thirds * (1.0 - d1 * d1 / d2),
                two_thirds * (1.0 - vdsat * vdsat / d2),
                0.0,
            )
        }
    }
}

/// The devices of this module in one circuit.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ExtDevices {
    pub mutual: Vec<Mutual>,
    pub poly: Vec<PolySource>,
    pub junctions: Vec<Junction>,
    pub bulk: BulkJunctions,
    pub meyer: Vec<MeyerCaps>,
}

impl ExtDevices {
    /// Whether any device here is nonlinear (Newton iterations needed).
    #[inline]
    pub fn is_nonlinear(&self) -> bool {
        !self.poly.is_empty()
            || !self.junctions.is_empty()
            || !self.bulk.is_empty()
            || !self.meyer.is_empty()
    }

    /// Anode / cathode pairs of every junction (full and plain).
    pub fn junction_terminals(&self) -> impl Iterator<Item = (usize, usize)> + '_ {
        self.junctions
            .iter()
            .map(|j| (j.anode, j.cathode))
            .chain((0..self.bulk.len()).map(|k| (self.bulk.anode[k], self.bulk.cathode[k])))
    }

    /// Whether any device here stores charge (a reactive state for the time step control).
    #[inline]
    pub fn stores_charge(&self) -> bool {
        !self.meyer.is_empty() || self.junctions.iter().any(Junction::stores_charge)
    }

    /// Starts every charge history at the solution `x` (no current flows through the charges).
    pub fn start_charges(&mut self, x: &[f64], vt: f64) {
        for j in self.junctions.iter_mut().filter(|j| j.stores_charge()) {
            let (q, _) = j.charge_at(j.voltage(x), vt);
            j.charge.start(q);
        }
        for mc in &mut self.meyer {
            mc.v_prev = mc.voltages(x);
            mc.c_prev = mc.capacitances(x);
            for st in &mut mc.charge {
                st.start(0.0);
            }
        }
    }

    /// Sets the history term of every charge for the companion coefficients `a1`, `a2` of the next step.
    pub fn set_charge_history(&mut self, a1: f64, a2: f64, uses_rate: bool) {
        for j in &mut self.junctions {
            j.charge.set_hist(a1, a2, uses_rate);
        }
        for mc in &mut self.meyer {
            for st in &mut mc.charge {
                st.set_hist(a1, a2, uses_rate);
            }
        }
    }

    /// Meyer charges at `x` measured from the last accepted point.
    #[inline]
    fn meyer_charges(mc: &MeyerCaps, x: &[f64]) -> ([f64; 3], [f64; 3], [f64; 3]) {
        let v = mc.voltages(x);
        let c = mc.capacitances(x);
        let mut q = [0.0; 3];
        let mut c_avg = [0.0; 3];
        for k in 0..3 {
            c_avg[k] = 0.5 * (c[k] + mc.c_prev[k]);
            q[k] = mc.charge[k].q_prev + c_avg[k] * (v[k] - mc.v_prev[k]);
        }
        (q, c_avg, c)
    }

    /// Commits the charges of the accepted solution `x` of a step with leading coefficient `a0`.
    pub fn commit_charges(&mut self, x: &[f64], a0: f64, vt: f64) {
        for j in self.junctions.iter_mut().filter(|j| j.stores_charge()) {
            let (q, _) = j.charge_at(j.voltage(x), vt);
            j.charge.commit(q, a0);
        }
        for mc in &mut self.meyer {
            let (q, _, c) = Self::meyer_charges(mc, x);
            for (k, st) in mc.charge.iter_mut().enumerate() {
                st.commit(q[k], a0);
            }
            mc.v_prev = mc.voltages(x);
            mc.c_prev = c;
        }
    }

    /// Calls `f(q, q_n, q_(n-1), q_(n-2), c)` for every charge at the solution `x` (`c`: its capacitance).
    pub fn for_each_charge(&self, x: &[f64], vt: f64, mut f: impl FnMut(f64, f64, f64, f64, f64)) {
        for j in self.junctions.iter().filter(|j| j.stores_charge()) {
            let (q, c) = j.charge_at(j.voltage(x), vt);
            let s = &j.charge;
            f(q, s.q_prev, s.q_prev2, s.q_prev3, c);
        }
        for mc in &self.meyer {
            let (q, c_avg, _) = Self::meyer_charges(mc, x);
            for (k, s) in mc.charge.iter().enumerate() {
                f(q[k], s.q_prev, s.q_prev2, s.q_prev3, c_avg[k]);
            }
        }
    }

    /// Makes the solution `x` the reference point of the junction step limiter.
    pub fn sync_limiter(&mut self, x: &[f64]) {
        for j in &mut self.junctions {
            j.vd = j.voltage(x);
        }
        for k in 0..self.bulk.len() {
            self.bulk.vd[k] = self.bulk.voltage(k, x);
        }
    }

    /// Fails if a junction of the accepted solution `x` needs the linear continuation of the exponential.
    pub fn check_exponent_range(&self, x: &[f64], vt: f64) -> SimResult<()> {
        let fail = |vd: f64, nvt: f64, name: &str| {
            Err(SimError::DeviceRangeExceeded(format!(
                "junction `{name}` voltage {vd:.6e} V is {:.1} thermal-voltage units (exponent argument limit {DIODE_EXP_ARG_MAX})",
                vd / nvt
            )))
        };
        for j in &self.junctions {
            let (vd, nvt) = (j.voltage(x), j.n * vt);
            if vd > DIODE_EXP_ARG_MAX * nvt {
                return fail(vd, nvt, &j.name);
            }
        }
        let b = &self.bulk;
        for k in 0..b.len() {
            let vd = b.voltage(k, x);
            if vd > DIODE_EXP_ARG_MAX * b.nvt[k] {
                return fail(vd, b.nvt[k], &b.names[k]);
            }
        }
        Ok(())
    }
}

impl DeviceSoaBuffer {
    /// Evaluates the devices of [`ExtDevices`] at `voltages` into `cond_buf` / `rhs_buf`; returns whether the
    /// junction step limiter changed a junction voltage.
    pub fn evaluate_ext_devices(&mut self, voltages: &[f64]) -> bool {
        let transient = self.is_transient;
        let a0 = self.transient_g_scale;
        let vt = self.temp_vt;
        let gmin = self.gmin;
        let cond = &mut self.cond_buf;
        let rhs = &mut self.rhs_buf;
        let ext = &mut self.ext;

        for mu in &ext.mutual {
            cond[mu.cond_buf_idx as usize] = if transient { mu.m * a0 } else { 0.0 };
        }

        for p in &ext.poly {
            let x = DeviceSoaBuffer::node_voltage(voltages, p.nc_pos)
                - DeviceSoaBuffer::node_voltage(voltages, p.nc_neg);
            let (f, df) = p.eval(x);
            cond[p.cond_buf_idx as usize] = df;
            rhs[p.rhs_buf_idx as usize] = f - df * x;
        }

        let mut limited = false;
        for j in &mut ext.junctions {
            let raw = j.voltage(voltages);
            let (nvt, vcrit) = (j.nvt, j.vcrit);
            let mut vd = pnjlim(raw, j.vd, nvt, vcrit);
            if vd < -j.xbv {
                // the same limiter mirrored at the breakdown knee
                let v_mirror = -(vd + j.xbv);
                let old_mirror = -(j.vd + j.xbv);
                vd = -(pnjlim(v_mirror, old_mirror, nvt, vcrit) + j.xbv);
            }
            if vd != raw && ((vd - raw).abs() > 1e-14 || vd / nvt > DIODE_EXP_ARG_MAX) {
                limited = true;
            }
            j.vd = vd;
            let (id, gd, _, _) = j.current(vd, vt);
            let (mut i, mut g) = (id + gmin * vd, gd + gmin);
            if transient && j.stores_charge() {
                let (q, c) = j.charge_at(vd, vt);
                i += a0 * q + j.charge.hist;
                g += a0 * c;
            }
            let ieq = i - g * vd;
            if j.fold_mos.is_some() {
                cond[j.cond_buf_idx as usize] += g;
                rhs[j.rhs_buf_idx as usize] += j.fold_sign * ieq;
            } else {
                cond[j.cond_buf_idx as usize] = g;
                rhs[j.rhs_buf_idx as usize] = ieq;
            }
        }

        limited |= ext.bulk.evaluate(voltages, gmin, cond, rhs);

        for mc in &ext.meyer {
            let v = mc.voltages(voltages);
            let (q, c_avg, _) = if transient {
                ExtDevices::meyer_charges(mc, voltages)
            } else {
                ([0.0; 3], [0.0; 3], [0.0; 3])
            };
            for k in 0..3 {
                let (g, ieq) = if transient {
                    let g = a0 * c_avg[k];
                    (g, a0 * q[k] + mc.charge[k].hist - g * v[k])
                } else {
                    (0.0, 0.0)
                };
                cond[mc.cond_buf_idx[k] as usize] = g;
                rhs[mc.rhs_buf_idx[k] as usize] = ieq;
            }
        }
        limited
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polynomial_and_slope() {
        let p = PolySource {
            coeffs: vec![1.0, 2.0, 0.5],
            ..Default::default()
        };
        assert_eq!(p.eval(2.0), (1.0 + 4.0 + 2.0, 2.0 + 2.0));
    }

    #[test]
    fn depletion_charge_is_the_integral_of_the_capacitance() {
        for &(m, fc) in &[(0.5, 0.5), (0.33, 0.1), (0.9, 0.95)] {
            let (cj0, vj) = (2e-12, 0.7);
            let mut prev = depletion_charge(-3.0, cj0, vj, m, fc);
            let h = 1e-4;
            let mut v = -3.0;
            while v < 1.0 {
                let mid = depletion_charge(v + 0.5 * h, cj0, vj, m, fc);
                let next = depletion_charge(v + h, cj0, vj, m, fc);
                let simpson = h / 6.0 * (prev.1 + 4.0 * mid.1 + next.1);
                assert!(
                    (next.0 - prev.0 - simpson).abs() < 1e-9 * cj0 * h + 1e-8 * simpson.abs(),
                    "m={m} fc={fc} v={v}"
                );
                prev = next;
                v += h;
            }
        }
    }

    #[test]
    fn breakdown_reaches_ibv_at_minus_bv() {
        let vt = 0.025864917007155805;
        let j = Junction {
            is: 1e-14,
            n: 2.0,
            xbv: Junction::fit_breakdown(Some(5.0), 1e-3, 1e-14, 2.0, vt),
            ikf: f64::INFINITY,
            vj: 1.0,
            cubic_reverse: true,
            ..Default::default()
        };
        let (i, ..) = j.current(-5.0, vt);
        assert!((i + 1e-3).abs() < 1e-15, "{i}");
    }

    #[test]
    fn knee_current_halves_at_ikf() {
        let vt = 0.025864917007155805;
        let is = 1e-14;
        let v = vt * (1e-3_f64 / is + 1.0).ln();
        let j = Junction {
            is,
            n: 1.0,
            xbv: f64::INFINITY,
            ikf: 1e-3,
            vj: 1.0,
            ..Default::default()
        };
        let (i, g, ..) = j.current(v, vt);
        assert!((i - 0.5e-3).abs() < 1e-15, "{i}");
        let (ih, ..) = j.current(v + 1e-7, vt);
        assert!(((ih - i) / 1e-7 - g).abs() < 1e-5 * g);
    }

    #[test]
    fn meyer_regions_are_continuous() {
        let (von, phi, cox) = (0.7, 0.6, 1.0);
        let at = |vgs: f64, vds: f64| meyer(vgs, vgs - vds, von, phi, cox);
        let eps = 1e-9;
        for &vgs in &[von - phi, von - 0.5 * phi, von] {
            let (a, b) = (at(vgs - eps, 0.5), at(vgs + eps, 0.5));
            assert!(
                (a.0 - b.0).abs() < 1e-6 && (a.2 - b.2).abs() < 1e-6,
                "{vgs}"
            );
        }
        // saturation boundary vds = vdsat
        let (a, b) = (at(1.7, 1.0 - eps), at(1.7, 1.0 + eps));
        assert!((a.0 - b.0).abs() < 1e-6 && (a.1 - b.1).abs() < 1e-6);
        // vds = 0: the channel charge splits evenly
        let (cgs, cgd, _) = at(2.0, 0.0);
        assert!((cgs - 0.5).abs() < 1e-12 && (cgd - 0.5).abs() < 1e-12);
    }
}
