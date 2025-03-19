use std::{cmp::Ordering, num::FpCategory};

use serde_derive::{Deserialize, Serialize};

macro_rules! impl_partialeq {
    ($t:ty) => {
        impl PartialEq for $t {
            // custom compare function because of floating point inaccuracy
            fn eq(&self, other: &$t) -> bool {
                if (self.0.classify() == FpCategory::Zero
                    || self.0.classify() == FpCategory::Normal)
                    && (other.0.classify() == FpCategory::Zero
                        || other.0.classify() == FpCategory::Normal)
                {
                    // four floating point numbers have to match
                    (self.0 * 1000.0).round() == (other.0 * 1000.0).round()
                } else {
                    false
                }
            }
        }
    };
}

macro_rules! impl_ord {
    ($t:ty) => {
        impl Ord for $t {
            // custom compare function to offer ordering
            fn cmp(&self, other: &$t) -> Ordering {
                if self.0 < other.0 {
                    Ordering::Less
                } else if self.0 > other.0 {
                    Ordering::Greater
                } else {
                    Ordering::Equal
                }
            }
        }
    };
}

/// Scale in millimeter
#[derive(Debug, Default, Copy, Clone, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Mm(pub f32);

impl Mm {
    pub fn into_pt(&self) -> Pt {
        let pt: Pt = (*self).into();
        pt
    }
}

impl From<Pt> for Mm {
    fn from(value: Pt) -> Mm {
        Mm(value.0 * 0.352_778_f32)
    }
}

impl Eq for Mm {}

impl_partialeq!(Mm);
impl_ord!(Mm);

/// Scale in point
#[derive(Debug, Default, Copy, Clone, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Pt(pub f32);

impl Pt {
    /// Convert this `Pt` value into `Px` at the given `dpi`.
    pub fn into_px(self, dpi: f32) -> Px {
        // First convert from points back to millimeters, then from millimeters to pixels.
        //
        // 1 mm = 2.834646 points
        // So, mm = pt / 2.834646
        // And from Px::into_pt we know:
        //     px -> mm = px * (25.4 / dpi)
        // So reversing it:
        //     mm -> px = mm * (dpi / 25.4)
        //
        let mm = self.0 / 2.834646_f32;
        let px_f = mm * (dpi / 25.4_f32);

        // Round to the nearest whole number of pixels before casting to `usize`.
        Px(px_f.round() as usize)
    }
}

impl From<Mm> for Pt {
    fn from(value: Mm) -> Pt {
        Pt(value.0 * 2.834_646_f32)
    }
}

impl From<Pt> for ::lopdf::Object {
    fn from(value: Pt) -> Self {
        Self::Real(value.0)
    }
}

impl Eq for Pt {}

impl_partialeq!(Pt);
impl_ord!(Pt);

/// Scale in pixels
#[derive(Debug, Default, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Px(pub usize);

impl Px {
    pub fn into_pt(self, dpi: f32) -> Pt {
        Mm(self.0 as f32 * (25.4 / dpi)).into()
    }
}

use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Sub, SubAssign};

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

macro_rules! impl_mul_f32 {
    ($type:ident) => {
        impl Mul<f32> for $type {
            type Output = Self;
            fn mul(self, other: f32) -> Self {
                Self { 0: self.0 * other }
            }
        }
    };
}

macro_rules! impl_mul_assign_f32 {
    ($type:ident) => {
        impl MulAssign<f32> for $type {
            fn mul_assign(&mut self, other: f32) {
                self.0 *= other;
            }
        }
    };
}

macro_rules! impl_div {
    ($type:ident) => {
        impl Div<$type> for $type {
            type Output = f32;
            fn div(self, other: $type) -> Self::Output {
                self.0 / other.0
            }
        }
        impl Div<f32> for $type {
            type Output = Self;
            fn div(self, other: f32) -> Self::Output {
                Self { 0: self.0 / other }
            }
        }
    };
}

macro_rules! impl_div_assign_f32 {
    ($type:ident) => {
        impl DivAssign<f32> for $type {
            fn div_assign(&mut self, other: f32) {
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

impl_mul_f32!(Mm);
impl_mul_f32!(Pt);

impl_mul_assign_f32!(Mm);
impl_mul_assign_f32!(Pt);

impl_div!(Mm);
impl_div!(Pt);

impl_div_assign_f32!(Mm);
impl_div_assign_f32!(Pt);

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
    assert_eq!(mm1, Pt(2.834_647_4));
    assert_eq!(mm2, Pt(65.1969));
}

#[test]
fn mm_eq_zero_check() {
    let mm1: Mm = Mm(0.0);
    let mm2: Mm = Mm(0.0);
    assert_eq!(mm1, mm2);
    assert_eq!(mm1, Mm(0.0));
    assert_eq!(mm2, Mm(0.0));
}

#[test]
fn max_mm() {
    let mm_vector = [Mm(0.0), Mm(1.0), Mm(2.0)];
    assert_eq!(mm_vector.iter().max().unwrap(), &Mm(2.0));
}

#[test]
fn min_mm() {
    let mm_vector = [Mm(0.0), Mm(1.0), Mm(2.0)];
    assert_eq!(mm_vector.iter().min().unwrap(), &Mm(0.0));
}

#[test]
fn pt_eq_zero_check() {
    let pt1: Pt = Pt(0.0);
    let pt2: Pt = Pt(0.0);
    assert_eq!(pt1, pt2);
    assert_eq!(pt1, Pt(0.0));
    assert_eq!(pt2, Pt(0.0));
}

#[test]
fn max_pt() {
    let pt_vector = [Pt(0.0), Pt(1.0), Pt(2.0)];
    assert_eq!(pt_vector.iter().max().unwrap(), &Pt(2.0));
}

#[test]
fn min_pt() {
    let pt_vector = [Pt(0.0), Pt(1.0), Pt(2.0)];
    assert_eq!(pt_vector.iter().min().unwrap(), &Pt(0.0));
}
