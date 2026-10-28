"""Independent oracles for the acc_ord_* decks.

None of these uses either simulator: closed forms (numpy, complex eigen-decomposition) or scipy ODE integrators
(Radau / DOP853 at rtol 1e-11..1e-13) on the circuit equations written out by hand.  Constants that are not in the
decks (the diode thermal voltage, GMIN) were calibrated against DC operating points of BOTH tools (see the check
scripts' headers); only observable output was used (AGENTS.md Rule 1).
"""
import numpy as np
from scipy.integrate import solve_ivp
from scipy.optimize import brentq
from scipy.special import lambertw

GMIN = 1e-12  # both tools put 1e-12 S across every junction / MOS channel (DC-calibrated, residual < 1e-11 V)
VT = 0.0258649170072  # kT/q at the tools' 300.15 K, fitted to the DC OP of a diode+R deck: ngspice residual 1e-16 V, RustSpice 4e-11 V


# ---------------------------------------------------------------- series RLC, sine drive (closed form)
class RlcSine:
    def __init__(self, R, L, C, A, f):
        self.R, self.L, self.C, self.A, self.w = R, L, C, A, 2 * np.pi * f

    def exact(self, t):
        """(i_L, v_C) at times t for zero initial state: phasor steady state + homogeneous solution."""
        R, L, C, A, w = self.R, self.L, self.C, self.A, self.w
        t = np.asarray(t, float)
        Z = R + 1j * w * L + 1 / (1j * w * C)
        I = A / Z
        V = I / (1j * w * C)
        ip = np.imag(I * np.exp(1j * w * t))
        vp = np.imag(V * np.exp(1j * w * t))
        M = np.array([[-R / L, -1 / L], [1 / C, 0]], complex)
        lam, W = np.linalg.eig(M)
        c = np.linalg.solve(W, -np.array([np.imag(I), np.imag(V)]).astype(complex))
        ih = np.zeros(t.shape, complex)
        vh = np.zeros(t.shape, complex)
        for k in range(2):
            e = c[k] * np.exp(lam[k] * t)
            ih += e * W[0, k]
            vh += e * W[1, k]
        return ip + ih.real, vp + vh.real

    def dop853(self, tg):
        R, L, C, A, w = self.R, self.L, self.C, self.A, self.w
        f = lambda t, x: [(A * np.sin(w * t) - R * x[0] - x[1]) / L, x[0] / C]
        s = solve_ivp(f, (0, tg[-1]), [0.0, 0.0], method="DOP853", rtol=1e-12, atol=1e-18, t_eval=tg)
        assert s.success
        return s.y[0], s.y[1]


# ---------------------------------------------------------------- half-wave rectifier (scalar ODE, Lambert-W reduction)
class Rectifier:
    """V1 in 0 SIN(0 A f); R1 in a R1; D1 a out (IS, N); C1 out 0 C; R2 out 0 R2.
    The algebraic node `a` is eliminated exactly: vd*(1+R1*GMIN) + R1*IS*exp(vd/nVt) = vin - vout + R1*IS."""

    def __init__(self, A, f, R1, R2, C, IS, N=1.0):
        self.A, self.w, self.R1, self.R2, self.C, self.IS, self.nVt = A, 2 * np.pi * f, R1, R2, C, IS, N * VT

    def vin(self, t):
        return self.A * np.sin(self.w * t)

    def vd(self, u):
        a = 1 + self.R1 * GMIN
        up = u + self.R1 * self.IS
        lz = np.log(self.R1 * self.IS / (self.nVt * a)) + up / (a * self.nVt)
        if lz < 600.0:
            W = lambertw(np.exp(lz)).real
        else:
            W = lz - np.log(lz)
            for _ in range(60):
                W -= (W + np.log(W) - lz) / (1 + 1 / W)
        return up / a - self.nVt * W

    def iD(self, vd):
        return self.IS * np.expm1(vd / self.nVt) + GMIN * vd

    def rhs(self, t, x):
        return [(self.iD(self.vd(self.vin(t) - x[0])) - x[0] / self.R2) / self.C]

    def solve(self, tg, method="Radau", rtol=1e-11, atol=1e-16):
        s = solve_ivp(self.rhs, (0, tg[-1]), [0.0], method=method, rtol=rtol, atol=atol, t_eval=tg)
        assert s.success, s.message
        return s.y[0]

    def anode(self, tg, vout):
        return vout + np.array([self.vd(self.vin(t) - v) for t, v in zip(tg, vout)])


# ---------------------------------------------------------------- lossless LC kicked by a current pulse (piecewise DOP853)
class LcPulse:
    """I1 0 n PULSE(0 I0 td tr tf pw 1); L1 n 0 L; C1 n 0 C.  state (v, iL)."""

    def __init__(self, L, C, I0, td, tr, tf, pw):
        self.L, self.C, self.I0, self.td, self.tr, self.tf, self.pw = L, C, I0, td, tr, tf, pw
        self.w0 = 1 / np.sqrt(L * C)
        self.T0 = 2 * np.pi / self.w0
        self.t_end = td + tr + pw + tf

    def isrc(self, t):
        td, tr, pw, tf, I0 = self.td, self.tr, self.pw, self.tf, self.I0
        if t < td:
            return 0.0
        if t < td + tr:
            return I0 * (t - td) / tr
        if t < td + tr + pw:
            return I0
        if t < self.t_end:
            return I0 * (1 - (t - td - tr - pw) / tf)
        return 0.0

    def energy(self, v, i):
        return 0.5 * self.C * np.asarray(v) ** 2 + 0.5 * self.L * np.asarray(i) ** 2

    def exact(self, tg):
        tg = np.asarray(tg, float)
        rhs = lambda t, x: [(self.isrc(t) - x[1]) / self.C, x[0] / self.L]
        bps = [0.0, self.td, self.td + self.tr, self.td + self.tr + self.pw, self.t_end, max(tg[-1], self.t_end)]
        x = np.array([0.0, 0.0])
        out = np.zeros((2, len(tg)))
        for a, b in zip(bps[:-1], bps[1:]):
            if b <= a:
                continue
            sel = np.where((tg >= a - 1e-15) & (tg <= b + 1e-15))[0]
            te = np.unique(np.concatenate([np.clip(tg[sel], a, b), [b]]))
            s = solve_ivp(rhs, (a, b), x, method="DOP853", rtol=1e-13, atol=1e-18, t_eval=te)
            assert s.success
            tt = np.clip(tg[sel], a, b)
            pos = np.clip(np.searchsorted(s.t, tt), 0, len(s.t) - 1)
            pos = np.where((pos > 0) & (abs(s.t[np.maximum(pos - 1, 0)] - tt) < abs(s.t[pos] - tt)), pos - 1, pos)
            out[:, sel] = s.y[:, pos]
            x = s.y[:, -1]
        return out[0], out[1]


# ---------------------------------------------------------------- CMOS inverter, SPICE level-1 equations, explicit load cap
def mos_ids(vgs, vds, kp, vt, W, L, lam):
    """Shichman-Hodges drain current (vds >= 0), lambda applied in triode and saturation, GMIN across the channel."""
    beta = kp * W / L
    vgst = vgs - vt
    if vgst <= 0:
        i = 0.0
    elif vds < vgst:
        i = beta * (vgst * vds - 0.5 * vds * vds) * (1 + lam * vds)
    else:
        i = 0.5 * beta * vgst * vgst * (1 + lam * vds)
    return i + GMIN * vds


class Inverter:
    def __init__(self, VDD, VIN_off, VIN_amp, f, CL, wn, wp, ln, lp, kpn, kpp, vtn, vtp, lam):
        self.VDD, self.off, self.amp, self.w, self.CL = VDD, VIN_off, VIN_amp, 2 * np.pi * f, CL
        self.n = (kpn, vtn, wn, ln, lam)
        self.p = (kpp, abs(vtp), wp, lp, lam)

    def vin(self, t):
        return self.off + self.amp * np.sin(self.w * t)

    def idn(self, vin, vo):
        kp, vt, W, L, lam = self.n
        return mos_ids(vin, vo, kp, vt, W, L, lam)

    def idp(self, vin, vo):
        kp, vt, W, L, lam = self.p
        return mos_ids(self.VDD - vin, self.VDD - vo, kp, vt, W, L, lam)

    def dc(self, vin):
        return brentq(lambda vo: self.idp(vin, vo) - self.idn(vin, vo), -1e-3, self.VDD + 1e-3, xtol=1e-15, rtol=1e-15)

    def solve(self, tg, method="Radau", rtol=1e-11, atol=1e-14):
        rhs = lambda t, x: [(self.idp(self.vin(t), x[0]) - self.idn(self.vin(t), x[0])) / self.CL]
        s = solve_ivp(rhs, (0, tg[-1]), [self.dc(self.vin(0.0))], method=method, rtol=rtol, atol=atol, t_eval=tg)
        assert s.success, s.message
        return s.y[0]

    def supply_current(self, tg, vout):
        """i(vdd) in the SPICE sign convention: current flowing INTO the + terminal = minus the PMOS source current."""
        return -np.array([self.idp(self.vin(t), v) for t, v in zip(tg, vout)])
