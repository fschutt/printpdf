//! PDF axial / radial shadings (CSS linear / radial gradients).
//!
//! Linear gradients map to a PDF **axial** shading (`ShadingType 2`) and radial
//! gradients to a **radial** shading (`ShadingType 3`). Both reference a PDF
//! *Function* mapping the parametric value `t ∈ [0,1]` to a color. A two-stop
//! gradient uses a single Type 2 ("exponential") function; a multi-stop gradient
//! uses a Type 3 ("stitching") function over per-segment Type 2 functions.
//!
//! All of these are plain dictionaries (no streams), so an entire shading +
//! function serializes as one self-contained dictionary placed directly in the
//! page's `/Shading` resource sub-dictionary (no extra indirect objects needed).
//!
//! The shading is painted with the `sh` operator ([`crate::Op::PaintShading`]),
//! which fills the current clip region — so the bridge clips to the element box
//! (optionally rounded) before painting.

use serde_derive::{Deserialize, Serialize};
use lopdf::Dictionary as LoDictionary;

/// Internal id for a `/Shading` resource (mirrors [`crate::XObjectId`] etc.).
#[derive(Debug, PartialEq, Clone, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ShadingId(pub String);

impl ShadingId {
    pub fn new() -> Self {
        Self(crate::utils::random_character_string_32())
    }
}

/// A single gradient color stop: `offset ∈ [0,1]` along the gradient axis and an
/// RGB color with components in `[0,1]`.
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct GradientStop {
    pub offset: f32,
    pub color: [f32; 3],
}

/// Geometry of a shading.
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub enum ShadingGeometry {
    /// Axial (linear) gradient between two points: `[x0, y0, x1, y1]` in PDF pt.
    Axial { coords: [f32; 4] },
    /// Radial gradient between two circles: `[x0, y0, r0, x1, y1, r1]` in PDF pt.
    Radial { coords: [f32; 6] },
}

/// A resolved PDF shading (gradient) ready to serialize to a dictionary.
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Shading {
    pub geometry: ShadingGeometry,
    /// Color stops, sorted by `offset` ascending. At least one entry.
    pub stops: Vec<GradientStop>,
    /// Whether to extend the shading before the first / after the last stop
    /// (CSS gradients clamp, so this is normally `(true, true)`).
    pub extend: (bool, bool),
}

impl Shading {
    /// Build the self-contained PDF Shading dictionary (with an inline Function).
    pub fn to_dict(&self) -> LoDictionary {
        use lopdf::Object::*;

        let (shading_type, coords): (i64, Vec<lopdf::Object>) = match &self.geometry {
            ShadingGeometry::Axial { coords } => (2, coords.iter().map(|v| Real(*v)).collect()),
            ShadingGeometry::Radial { coords } => (3, coords.iter().map(|v| Real(*v)).collect()),
        };

        let mut dict = LoDictionary::new();
        dict.set("ShadingType", Integer(shading_type));
        dict.set("ColorSpace", Name(b"DeviceRGB".to_vec()));
        dict.set("Coords", Array(coords));
        dict.set("Function", self.build_function());
        dict.set(
            "Extend",
            Array(vec![Boolean(self.extend.0), Boolean(self.extend.1)]),
        );
        dict
    }

    /// Build the PDF Function mapping `t ∈ [0,1]` → RGB.
    ///
    /// The stops are turned into a list of linear segments `(t_start, t_end,
    /// c_start, c_end)`. A leading constant segment `[0, first_offset]` and a
    /// trailing constant segment `[last_offset, 1]` are inserted when the stops
    /// do not span the full `[0,1]` range, so e.g. `linear-gradient(red 30%,
    /// blue)` renders solid red up to 30% (matching CSS) rather than
    /// interpolating from `t = 0`.
    fn build_function(&self) -> lopdf::Object {
        let n = self.stops.len();
        if n == 0 {
            return lopdf::Object::Dictionary(exponential_fn([0.0; 3], [0.0; 3]));
        }
        if n == 1 {
            let c = self.stops[0].color;
            return lopdf::Object::Dictionary(exponential_fn(c, c));
        }

        // (t_start, t_end, c_start, c_end) per segment.
        let mut segs: Vec<(f32, f32, [f32; 3], [f32; 3])> = Vec::new();
        let first = &self.stops[0];
        let last = &self.stops[n - 1];
        let o_first = first.offset.clamp(0.0, 1.0);
        let o_last = last.offset.clamp(0.0, 1.0);

        if o_first > 0.0 {
            segs.push((0.0, o_first, first.color, first.color));
        }
        for w in self.stops.windows(2) {
            let ta = w[0].offset.clamp(0.0, 1.0);
            let tb = w[1].offset.clamp(0.0, 1.0);
            if tb > ta {
                segs.push((ta, tb, w[0].color, w[1].color));
            }
        }
        if o_last < 1.0 {
            segs.push((o_last, 1.0, last.color, last.color));
        }

        if segs.is_empty() {
            // All stops at the same offset → constant color.
            let c = first.color;
            return lopdf::Object::Dictionary(exponential_fn(c, c));
        }
        if segs.len() == 1 {
            let s = &segs[0];
            return lopdf::Object::Dictionary(exponential_fn(s.2, s.3));
        }

        use lopdf::Object::*;
        let functions: Vec<lopdf::Object> = segs
            .iter()
            .map(|s| Dictionary(exponential_fn(s.2, s.3)))
            .collect();
        // Interior breakpoints = the end of every segment except the last.
        let bounds: Vec<lopdf::Object> = segs[..segs.len() - 1]
            .iter()
            .map(|s| Real(s.1))
            .collect();
        // Each segment maps its sub-domain linearly onto its function's [0,1].
        let encode: Vec<lopdf::Object> = segs.iter().flat_map(|_| [Real(0.0), Real(1.0)]).collect();

        let mut dict = LoDictionary::new();
        dict.set("FunctionType", Integer(3));
        dict.set("Domain", Array(vec![Real(0.0), Real(1.0)]));
        dict.set("Functions", Array(functions));
        dict.set("Bounds", Array(bounds));
        dict.set("Encode", Array(encode));
        Dictionary(dict)
    }
}

/// A Type 2 ("exponential interpolation") function from `c0` to `c1` over `[0,1]`.
fn exponential_fn(c0: [f32; 3], c1: [f32; 3]) -> LoDictionary {
    use lopdf::Object::*;
    let mut dict = LoDictionary::new();
    dict.set("FunctionType", Integer(2));
    dict.set("Domain", Array(vec![Real(0.0), Real(1.0)]));
    dict.set("C0", Array(c0.iter().map(|v| Real(*v)).collect()));
    dict.set("C1", Array(c1.iter().map(|v| Real(*v)).collect()));
    dict.set("N", Real(1.0));
    dict
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn axial_two_stop_builds_type2_function() {
        let sh = Shading {
            geometry: ShadingGeometry::Axial { coords: [0.0, 0.0, 100.0, 0.0] },
            stops: vec![
                GradientStop { offset: 0.0, color: [1.0, 0.0, 0.0] },
                GradientStop { offset: 1.0, color: [0.0, 0.0, 1.0] },
            ],
            extend: (true, true),
        };
        let d = sh.to_dict();
        assert_eq!(d.get(b"ShadingType").unwrap().as_i64().unwrap(), 2);
        let f = d.get(b"Function").unwrap().as_dict().unwrap();
        assert_eq!(f.get(b"FunctionType").unwrap().as_i64().unwrap(), 2);
    }

    #[test]
    fn multi_stop_builds_stitching_function() {
        let sh = Shading {
            geometry: ShadingGeometry::Axial { coords: [0.0, 0.0, 0.0, 100.0] },
            stops: vec![
                GradientStop { offset: 0.0, color: [1.0, 0.0, 0.0] },
                GradientStop { offset: 0.5, color: [0.0, 1.0, 0.0] },
                GradientStop { offset: 1.0, color: [0.0, 0.0, 1.0] },
            ],
            extend: (true, true),
        };
        let d = sh.to_dict();
        let f = d.get(b"Function").unwrap().as_dict().unwrap();
        assert_eq!(f.get(b"FunctionType").unwrap().as_i64().unwrap(), 3);
        // 2 segments → 2 functions, 1 interior bound.
        assert_eq!(f.get(b"Functions").unwrap().as_array().unwrap().len(), 2);
        assert_eq!(f.get(b"Bounds").unwrap().as_array().unwrap().len(), 1);
    }

    #[test]
    fn leading_constant_segment_for_offset_gradient() {
        // red 30% .. blue 100%  → a constant [0,0.3] segment + the real one.
        let sh = Shading {
            geometry: ShadingGeometry::Axial { coords: [0.0, 0.0, 100.0, 0.0] },
            stops: vec![
                GradientStop { offset: 0.3, color: [1.0, 0.0, 0.0] },
                GradientStop { offset: 1.0, color: [0.0, 0.0, 1.0] },
            ],
            extend: (true, true),
        };
        let d = sh.to_dict();
        let f = d.get(b"Function").unwrap().as_dict().unwrap();
        assert_eq!(f.get(b"FunctionType").unwrap().as_i64().unwrap(), 3);
        // constant[0,0.3] + interp[0.3,1.0] = 2 functions, bound at 0.3.
        assert_eq!(f.get(b"Functions").unwrap().as_array().unwrap().len(), 2);
        let bounds = f.get(b"Bounds").unwrap().as_array().unwrap();
        assert_eq!(bounds.len(), 1);
        assert!((bounds[0].as_float().unwrap() - 0.3).abs() < 1e-4);
    }
}
