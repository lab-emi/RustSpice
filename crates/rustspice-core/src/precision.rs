//! Numerical precision primitives:
//! - [`Df64`]: Unevaluated sum of two IEEE-754 `f32` values (`hi + lo`) providing ~48 bits
//!   of significand precision (~14.5 decimal digits) via hardware `fma` for Apple Metal and WebGPU.
//! - [`Dual`]: Forward-mode automatic differentiation number $x + \epsilon x'$ for exact Jacobian
//!   and sensitivity verification.

use std::ops::{Add, AddAssign, Div, Mul, Neg, Sub, SubAssign};

use serde::{Deserialize, Serialize};

/// High-precision Double-Float (`df64`) representation using an unevaluated sum
/// of two IEEE-754 single-precision floats (`hi + lo`).
///
/// Designed for GPU backends lacking native `f64` ALUs (e.g., Apple Metal, WebGPU)
/// or consumer GPUs with throttled `f64` throughput, delivering ~48 bits of significand
/// precision via hardware fused multiply-add (`fma`).
#[repr(C, align(8))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Df64 {
    pub hi: f32,
    pub lo: f32,
}

impl Df64 {
    pub const ZERO: Self = Self { hi: 0.0, lo: 0.0 };
    pub const ONE: Self = Self { hi: 1.0, lo: 0.0 };

    /// Splits a double-precision `f64` into an exact `(hi, lo)` pair of `f32` components.
    #[inline]
    pub fn from_f64(x: f64) -> Self {
        let hi = x as f32;
        let lo = (x - f64::from(hi)) as f32;
        Self { hi, lo }
    }

    /// Reconstructs the `f64` value from `hi + lo`.
    #[inline]
    pub fn to_f64(self) -> f64 {
        f64::from(self.hi) + f64::from(self.lo)
    }

    /// Knuth's `QuickTwoSum(a, b)` assuming `|a| >= |b|`.
    #[inline]
    pub fn quick_two_sum(a: f32, b: f32) -> Self {
        let s = a + b;
        let e = b - (s - a);
        Self { hi: s, lo: e }
    }

    /// Knuth's error-free `TwoSum(a, b)` without magnitude assumptions.
    #[inline]
    pub fn two_sum(a: f32, b: f32) -> Self {
        let s = a + b;
        let v = s - a;
        let e = (a - (s - v)) + (b - v);
        Self { hi: s, lo: e }
    }

    /// Error-free product `TwoProd(a, b)` utilizing hardware fused multiply-add (`fma`).
    #[inline]
    pub fn two_prod(a: f32, b: f32) -> Self {
        let p = a * b;
        let e = a.mul_add(b, -p);
        Self { hi: p, lo: e }
    }

    #[inline]
    pub fn abs(self) -> Self {
        if self.hi < 0.0 || (self.hi == 0.0 && self.lo < 0.0) {
            -self
        } else {
            self
        }
    }

    #[inline]
    pub fn sqrt(self) -> Self {
        if self.hi <= 0.0 {
            return Self::ZERO;
        }
        let x = self.hi.sqrt();
        let x_sq = Self::two_prod(x, x);
        let rem = self - x_sq;
        let corr = rem.hi / (2.0 * x);
        Self::quick_two_sum(x, corr)
    }

    #[inline]
    pub fn exp(self) -> Self {
        let x = self.to_f64().clamp(-80.0, 80.0);
        Self::from_f64(x.exp())
    }

    #[inline]
    pub fn ln(self) -> Self {
        let x = self.to_f64().max(1e-30);
        Self::from_f64(x.ln())
    }

    #[inline]
    pub fn fma(self, b: Self, c: Self) -> Self {
        (self * b) + c
    }
}

impl From<f64> for Df64 {
    #[inline]
    fn from(value: f64) -> Self {
        Self::from_f64(value)
    }
}

impl From<Df64> for f64 {
    #[inline]
    fn from(value: Df64) -> Self {
        value.to_f64()
    }
}

impl Neg for Df64 {
    type Output = Self;
    #[inline]
    fn neg(self) -> Self::Output {
        Self {
            hi: -self.hi,
            lo: -self.lo,
        }
    }
}

impl Add for Df64 {
    type Output = Self;
    #[inline]
    fn add(self, rhs: Self) -> Self::Output {
        let s = Self::two_sum(self.hi, rhs.hi);
        let t = Self::two_sum(self.lo, rhs.lo);
        let c = s.lo + t.hi;
        let v = Self::quick_two_sum(s.hi, c);
        let w = t.lo + v.lo;
        Self::quick_two_sum(v.hi, w)
    }
}

impl AddAssign for Df64 {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl Sub for Df64 {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: Self) -> Self::Output {
        self + (-rhs)
    }
}

impl SubAssign for Df64 {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}

impl Mul for Df64 {
    type Output = Self;
    #[inline]
    fn mul(self, rhs: Self) -> Self::Output {
        let mut p = Self::two_prod(self.hi, rhs.hi);
        p.lo = self.hi.mul_add(rhs.lo, self.lo.mul_add(rhs.hi, p.lo));
        Self::quick_two_sum(p.hi, p.lo)
    }
}

impl Div for Df64 {
    type Output = Self;
    #[inline]
    fn div(self, rhs: Self) -> Self::Output {
        let q1 = self.hi / rhs.hi;
        let r = self - (rhs * Self { hi: q1, lo: 0.0 });
        let q2 = r.hi / rhs.hi;
        let r2 = r - (rhs * Self { hi: q2, lo: 0.0 });
        let q3 = r2.hi / rhs.hi;
        let out = Self::quick_two_sum(q1, q2);
        out + Self { hi: q3, lo: 0.0 }
    }
}

/// Forward-mode Dual number $v + d \cdot \epsilon$ with $\epsilon^2 = 0$.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Dual {
    pub val: f64,
    pub deriv: f64,
}

impl Dual {
    #[inline]
    pub const fn new(val: f64, deriv: f64) -> Self {
        Self { val, deriv }
    }

    #[inline]
    pub const fn constant(val: f64) -> Self {
        Self { val, deriv: 0.0 }
    }

    #[inline]
    pub const fn variable(val: f64) -> Self {
        Self { val, deriv: 1.0 }
    }

    #[inline]
    pub fn exp(self) -> Self {
        let e = self.val.clamp(-80.0, 80.0).exp();
        Self {
            val: e,
            deriv: e * self.deriv,
        }
    }

    #[inline]
    pub fn ln(self) -> Self {
        let x = self.val.max(1e-30);
        Self {
            val: x.ln(),
            deriv: self.deriv / x,
        }
    }

    #[inline]
    pub fn sqrt(self) -> Self {
        let s = self.val.max(0.0).sqrt();
        let d = if s > 0.0 { self.deriv / (2.0 * s) } else { 0.0 };
        Self { val: s, deriv: d }
    }
}

impl Add for Dual {
    type Output = Self;
    #[inline]
    fn add(self, rhs: Self) -> Self::Output {
        Self {
            val: self.val + rhs.val,
            deriv: self.deriv + rhs.deriv,
        }
    }
}

impl Sub for Dual {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            val: self.val - rhs.val,
            deriv: self.deriv - rhs.deriv,
        }
    }
}

impl Mul for Dual {
    type Output = Self;
    #[inline]
    fn mul(self, rhs: Self) -> Self::Output {
        Self {
            val: self.val * rhs.val,
            deriv: self.deriv.mul_add(rhs.val, self.val * rhs.deriv),
        }
    }
}

impl Div for Dual {
    type Output = Self;
    #[inline]
    fn div(self, rhs: Self) -> Self::Output {
        let inv = 1.0 / rhs.val;
        Self {
            val: self.val * inv,
            deriv: (self.deriv * rhs.val - self.val * rhs.deriv) * (inv * inv),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_df64_precision_exceeds_f32() {
        let a = std::f64::consts::PI;
        let b = std::f64::consts::E;
        let da = Df64::from_f64(a);
        let db = Df64::from_f64(b);

        let prod = (da * db).to_f64();
        let exact = a * b;
        assert!(
            (prod - exact).abs() < 1e-13,
            "Df64 product error: {:.3e}",
            (prod - exact).abs()
        );

        let quot = (da / db).to_f64();
        let exact_q = a / b;
        assert!(
            (quot - exact_q).abs() < 1e-13,
            "Df64 division error: {:.3e}",
            (quot - exact_q).abs()
        );

        let sq = da.sqrt().to_f64();
        assert!((sq - a.sqrt()).abs() < 1e-13);
    }
}
