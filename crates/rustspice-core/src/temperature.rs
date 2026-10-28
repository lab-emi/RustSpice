//! Temperature dependence of device parameters (textbook SPICE laws, derived from the semiconductor physics):
//!
//! * thermal voltage `Vt(T) = k T / q`;
//! * silicon band gap (Varshni) `Eg(T) = 1.16 - 7.02e-4 T^2 / (T + 1108)` eV;
//! * diode saturation current `IS(T) = IS (T/Tnom)^(XTI/N) exp((T/Tnom - 1) EG / (N Vt(T)))`;
//! * junction potential `VJ(T) = VJ T/Tnom - 3 Vt(T) ln(T/Tnom) - Eg(Tnom) T/Tnom + Eg(T)` (from
//!   `VJ = Vt ln(Na Nd / ni^2)` with `ni^2 ~ T^3 exp(-Eg/kT)`) and zero-bias capacitance
//!   `CJO(T) = CJO (1 + M (4e-4 (T - Tnom) - VJ(T)/VJ + 1))`;
//! * MOS level 1: mobility `KP(T) = KP (T/Tnom)^-1.5`, surface potential `PHI(T)` by the same law as `VJ(T)`
//!   (`PHI = 2 Vt ln(Nsub/ni)`), and threshold through the built-in voltage `VBI = VTO - type GAMMA sqrt(PHI)`:
//!   `VBI(T) = VBI + type (PHI(T) - PHI)/2 + (Eg(Tnom) - Eg(T))/2` (flat-band shift of the work-function difference
//!   with the Fermi level of the substrate), `VTO(T) = VBI(T) + type GAMMA sqrt(PHI(T))`;
//! * linear elements `X(T) = X (1 + TC1 dT + TC2 dT^2)`, `dT = T - Tnom`.
//!
//! All temperatures here are kelvin; the netlist stores degrees Celsius.

/// 0 °C in kelvin.
pub const KELVIN: f64 = 273.15;
/// Default circuit and nominal temperature, °C.
pub const DEFAULT_TEMP_C: f64 = 27.0;
/// Reference temperature of [`VT_REF`], kelvin.
pub const T_REF: f64 = 300.15;
/// `k T / q` at [`T_REF`] (the value the solvers have always used at 27 °C).
pub const VT_REF: f64 = 0.025864917007155805;

/// Thermal voltage at `t` kelvin (exactly [`VT_REF`] at 300.15 K).
#[inline]
pub fn thermal_voltage(t: f64) -> f64 {
    VT_REF * (t / T_REF)
}

/// Silicon band gap in eV at `t` kelvin.
#[inline]
pub fn silicon_gap(t: f64) -> f64 {
    1.16 - 7.02e-4 * t * t / (t + 1108.0)
}

/// Saturation current at `t`, given its value `is` at `tnom` (kelvin).
pub fn diode_saturation_current(is: f64, n: f64, eg: f64, xti: f64, t: f64, tnom: f64) -> f64 {
    if t == tnom {
        return is;
    }
    let ratio = t / tnom;
    let nvt = n * thermal_voltage(t);
    is * ratio.powf(xti / n) * ((ratio - 1.0) * eg / nvt).exp()
}

/// Built-in junction / surface potential at `t`, given its value `p` at `tnom` (kelvin).
pub fn junction_potential(p: f64, t: f64, tnom: f64) -> f64 {
    if t == tnom {
        return p;
    }
    let ratio = t / tnom;
    p * ratio - 3.0 * thermal_voltage(t) * ratio.ln() - silicon_gap(tnom) * ratio + silicon_gap(t)
}

/// Zero-bias junction capacitance at `t`, given `cjo`, grading `m` and potential `vj` at `tnom` (kelvin).
pub fn junction_capacitance(cjo: f64, m: f64, vj: f64, t: f64, tnom: f64) -> f64 {
    if t == tnom || cjo == 0.0 {
        return cjo;
    }
    let vj_t = junction_potential(vj, t, tnom);
    cjo * (1.0 + m * (4e-4 * (t - tnom) - vj_t / vj + 1.0))
}

/// Level-1 MOS parameters `(KP, VTO, PHI)` at `t`, from their values at `tnom` (kelvin). `sign` is +1 for NMOS,
/// -1 for PMOS; `vto` is the signed model threshold.
pub fn level1_at(
    kp: f64,
    vto: f64,
    phi: f64,
    gamma: f64,
    sign: f64,
    t: f64,
    tnom: f64,
) -> (f64, f64, f64) {
    if t == tnom {
        return (kp, vto, phi);
    }
    let ratio = t / tnom;
    let kp_t = kp * ratio.powf(-1.5);
    let phi_t = junction_potential(phi, t, tnom);
    let vbi = vto - sign * gamma * phi.max(0.0).sqrt();
    let vbi_t = vbi + sign * (phi_t - phi) / 2.0 + (silicon_gap(tnom) - silicon_gap(t)) / 2.0;
    let vto_t = vbi_t + sign * gamma * phi_t.max(0.0).sqrt();
    (kp_t, vto_t, phi_t)
}

/// Polynomial temperature factor of a linear element.
#[inline]
pub fn tc_factor(tc1: f64, tc2: f64, dt: f64) -> f64 {
    1.0 + tc1 * dt + tc2 * dt * dt
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_point_is_exact() {
        assert_eq!(thermal_voltage(T_REF), VT_REF);
        assert_eq!(
            diode_saturation_current(1e-14, 1.0, 1.11, 3.0, 300.15, 300.15),
            1e-14
        );
        assert_eq!(
            level1_at(1e-4, 0.7, 0.65, 0.4, 1.0, 300.15, 300.15),
            (1e-4, 0.7, 0.65)
        );
    }

    #[test]
    fn level1_threshold_at_100c() {
        // NMOS VTO 0.7 V, PHI 0.65 V, GAMMA 0 at 100 °C: square-law threshold 0.633 V (bench deck
        // cap_dev_mos_temp_card, independent closed form)
        let (kp, vto, _) = level1_at(100e-6, 0.7, 0.65, 0.0, 1.0, 373.15, 300.15);
        assert!((vto - 0.6329).abs() < 1e-4, "{vto}");
        assert!((kp / 100e-6 - (373.15f64 / 300.15).powf(-1.5)).abs() < 1e-15);
        // PMOS mirrors the shift of |VTO| for GAMMA = 0 only through the band-gap term sign
        let (_, vto_p, _) = level1_at(100e-6, -0.7, 0.65, 0.0, -1.0, 373.15, 300.15);
        assert!(vto_p < 0.0);
    }

    #[test]
    fn saturation_current_grows_with_temperature() {
        let is85 = diode_saturation_current(1e-14, 1.0, 1.11, 3.0, 358.15, 300.15);
        assert!(is85 > 1.7e-11 && is85 < 1.8e-11, "{is85}");
    }
}
