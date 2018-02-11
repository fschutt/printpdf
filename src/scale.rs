//! Scaling types for reducing errors between conversions between point (pt) and millimeter (mm)

macro_rules! impl_partialeq {
    ($t:ty) => (
        impl PartialEq for $t {
            // custom compare function because of floating point inaccuracy
            fn eq(&self, other: &$t) -> bool {

                if self.0.is_normal() && other.0.is_normal(){
                    // four floating point numbers have to match
                    (self.0 * 1000.0).round() == (other.0 * 1000.0).round()
                } else {
                    false
                }
            }
        }
    )
}


/// Scale in millimeter
#[derive(Debug, Copy, Clone, PartialOrd)]
pub struct Mm(pub f64);

impl Into<Pt> for Mm {
    fn into(self) -> Pt {
        Pt(self.0 * 2.834_646_f64)
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

impl_partialeq!(Pt);

/// Scale in pixels
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct Px(pub usize);

impl Px {
    pub fn into_pt(self, dpi: f64) -> Pt {
        Mm(self.0 as f64 * (25.4 / dpi)).into()
    }
}

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