//! Scaling types for reducing errors between conversions between point (pt) and millimeter (mm)

macro_rules! impl_partialeq {
    ($t:ty) => {
        impl PartialEq for $t {
            // custom compare function because of floating point inaccuracy
            fn eq(&self, other: &$t) -> bool {
                if self.0.is_normal() && other.0.is_normal() {
                    // four floating point numbers have to match
                    (self.0 * 1000.0).round() == (other.0 * 1000.0).round()
                } else {
                    false
                }
            }
        }
    };
}

/// Scale in millimeter
#[derive(Debug, Copy, Clone, PartialOrd)]
pub struct Mm(pub f64);

impl Into<Pt> for Mm {
    fn into(self) -> Pt {
        Pt(self.0 * 2.834_646_f64)
    }
}

impl Div<Mm> for Mm {
    type Output = f64;

    fn div(self, rhs: Mm) -> f64 {
        self.0 / rhs.0
    }
}

impl_partialeq!(Mm);

/// Scale in point
#[derive(Debug, Copy, Clone, PartialOrd)]
pub struct Pt(pub f64);

impl Into<Mm> for Pt {
    fn into(self) -> Mm {
        Mm(self.0 * 0.352_778_f64)
    }
}

impl Into<::lopdf::Object> for Pt {
    fn into(self) -> ::lopdf::Object {
        ::lopdf::Object::Real(self.0)
    }
}

impl Div<Pt> for Pt {
    type Output = f64;

    fn div(self, rhs: Pt) -> f64 {
        self.0 / rhs.0
    }
}

impl_partialeq!(Pt);

/// Scale in pixels
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct Px(pub usize);

impl Px {
    pub fn into_pt(self, dpi: f64) -> Pt {
        Mm(self.0 as f64 * (25.4 / dpi)).into()
    }
}

use std::ops::{Add, Div, Mul, Sub};
use std::ops::{AddAssign, DivAssign, MulAssign, SubAssign};

macro_rules! impl_add_self {
    ($type:ident) => {
        impl Add for $type {
            type Output = Self;
            fn add(self, other: Self) -> Self {
                Self {
                    0: self.0 + other.0,
                }
            }
        }
    };
}

macro_rules! impl_add_assign_self {
    ($type:ident) => {
        impl AddAssign for $type {
            fn add_assign(&mut self, other: Self) {
                self.0 += other.0;
            }
        }
    };
}

macro_rules! impl_sub_assign_self {
    ($type:ident) => {
        impl SubAssign for $type {
            fn sub_assign(&mut self, other: Self) {
                self.0 -= other.0;
            }
        }
    };
}

macro_rules! impl_sub_self {
    ($type:ident) => {
        impl Sub for $type {
            type Output = Self;
            fn sub(self, other: Self) -> Self {
                Self {
                    0: self.0 - other.0,
                }
            }
        }
    };
}

macro_rules! impl_mul_f64 {
    ($type:ident) => {
        impl Mul<f64> for $type {
            type Output = Self;
            fn mul(self, other: f64) -> Self {
                Self { 0: self.0 * other }
            }
        }
    };
}

macro_rules! impl_mul_assign_f64 {
    ($type:ident) => {
        impl MulAssign<f64> for $type {
            fn mul_assign(&mut self, other: f64) {
                self.0 *= other;
            }
        }
    };
}

macro_rules! impl_div {
    ($type:ident) => {
        impl Div<$type> for $type {
            type Output = f64;
            fn div(self, other: $type) -> Self::Output {
                self.0 / other.0
            }
        }
        impl Div<f64> for $type {
            type Output = Self;
            fn div(self, other: f64) -> Self::Output {
                Self { 0: self.0 / other }
            }
        }
    };
}

macro_rules! impl_div_assign_f64 {
    ($type:ident) => {
        impl DivAssign<f64> for $type {
            fn div_assign(&mut self, other: f64) {
                self.0 /= other;
            }
        }
    };
}

impl_add_self!(Mm);
impl_add_self!(Pt);
impl_add_self!(Px);

impl_add_assign_self!(Mm);
impl_add_assign_self!(Pt);
impl_add_assign_self!(Px);

impl_sub_assign_self!(Mm);
impl_sub_assign_self!(Pt);
impl_sub_assign_self!(Px);

impl_sub_self!(Mm);
impl_sub_self!(Pt);
impl_sub_self!(Px);

impl_mul_f64!(Mm);
impl_mul_f64!(Pt);

impl_mul_assign_f64!(Mm);
impl_mul_assign_f64!(Pt);

impl_div!(Mm);
impl_div!(Pt);

impl_div_assign_f64!(Mm);
impl_div_assign_f64!(Pt);

#[test]
fn point_to_mm_conversion() {
    let pt1: Mm = Pt(1.0).into();
    let pt2: Mm = Pt(15.0).into();
    assert_eq!(pt1, Mm(0.352778));
    assert_eq!(pt2, Mm(5.29167));
}

#[test]
fn mm_to_point_conversion() {
    let mm1: Pt = Mm(1.0).into();
    let mm2: Pt = Mm(23.0).into();
    assert_eq!(mm1, Pt(2.83464745483286));
    assert_eq!(mm2, Pt(65.1969));
}
