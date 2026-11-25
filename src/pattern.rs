//! Patterns: a repeated drawing or a gradient used as a fill or stroke color.
//!
//! A PDF paints with a *color*, which is normally a flat device color (`rg`, `k`, `g`). A
//! *pattern* is a color that varies over the page. It is selected with the `Pattern` color space
//! (`/Pattern cs`) and a name (`/P1 scn`), and then used by every fill or stroke until the color
//! is changed (ISO 32000-1, 8.7). There are two kinds:
//!
//! - A *tiling pattern* ([`Pattern::Tiling`]) is a small drawing, the *cell*, that is repeated
//!   in a grid to cover the painted area. It has a list of [`Op`]s of its own, a bounding box,
//!   the distance between two cells (`xstep`, `ystep`) and a matrix. A *colored* tiling pattern
//!   has its colors in its ops. An *uncolored* one has only a shape: the ops must not set a
//!   color, and the color comes from the [`Op::SetFillPattern`] or [`Op::SetOutlinePattern`]
//!   that selects it.
//! - A *shading pattern* ([`Pattern::Shading`]) is a [`Shading`] (an axial or radial gradient)
//!   used as a color. Unlike [`Op::PaintShading`], which paints the whole clip region, it fills
//!   the shape that is being filled, and it can be used for strokes and text as well.
//!
//! [`Pattern::External`] holds a pattern that is read from a PDF and cannot be modelled, such as
//! a mesh shading or a tiling pattern with nested resources. It is written back as it was read.
//!
//! # Coordinates
//!
//! The matrix of a pattern maps the pattern's own coordinate system to the default coordinate
//! system of the page (or of the form that uses the pattern). Patterns do not follow the `cm`
//! operator: moving the current transformation matrix does not move the pattern (ISO 32000-1,
//! 8.7.3.1).
//!
//! # Examples
//!
//! A page with a red and white checker pattern as the fill of a rectangle:
//!
//! ```
//! use printpdf::*;
//!
//! // A rectangle that is filled when it is drawn.
//! let fill = |x: f32, y: f32, w: f32, h: f32| Op::DrawRectangle {
//!     rectangle: Rect {
//!         mode: Some(PaintMode::Fill),
//!         ..Rect::from_xywh(Pt(x), Pt(y), Pt(w), Pt(h))
//!     },
//! };
//!
//! // One 10 x 10 pt cell: two red squares.
//! let red = Color::Rgb(Rgb::new(0.8, 0.1, 0.1, None));
//! let cell = vec![
//!     Op::SetFillColor { col: red },
//!     fill(0.0, 0.0, 5.0, 5.0),
//!     fill(5.0, 5.0, 5.0, 5.0),
//! ];
//! let pattern = Pattern::colored_tiling(Rect::from_wh(Pt(10.0), Pt(10.0)), 10.0, 10.0, cell);
//!
//! let mut doc = PdfDocument::new("pattern");
//! let id = doc.add_pattern(pattern);
//! let ops = vec![Op::SetFillPattern { id, color: None }, fill(50.0, 50.0, 200.0, 100.0)];
//! doc.pages.push(PdfPage::new(Mm(100.0), Mm(100.0), ops));
//! let bytes = doc.save(&PdfSaveOptions::default(), &mut Vec::new());
//! assert!(bytes.starts_with(b"%PDF"));
//! ```

use serde_derive::{Deserialize, Serialize};

use crate::{
    graphics::Rect, Color, DictItem, ExtendedGraphicsState, Op, PdfResources, Shading,
};

/// Id of a pattern in the document's resources, as returned by `PdfDocument::add_pattern`.
///
/// The wrapped string is the resource name used in the PDF.
#[derive(Debug, PartialEq, Clone, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PatternId(pub String);

impl PatternId {
    /// Creates a new random id.
    pub fn new() -> Self {
        Self(crate::utils::random_character_string_32())
    }
}

impl Default for PatternId {
    fn default() -> Self {
        Self::new()
    }
}

/// Where the color of a tiling pattern comes from (`/PaintType`, ISO 32000-1, 8.7.3.2).
#[derive(Debug, Default, PartialEq, Eq, Copy, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PatternPaintType {
    /// The ops of the pattern set their own colors (`/PaintType 1`).
    #[default]
    Colored,
    /// The ops only draw shapes (`/PaintType 2`). The color is the `color` of the op that
    /// selects the pattern. Color operators in the ops are ignored by viewers.
    Uncolored,
}

impl PatternPaintType {
    /// The number written as `/PaintType`.
    pub(crate) fn number(&self) -> i64 {
        match self {
            PatternPaintType::Colored => 1,
            PatternPaintType::Uncolored => 2,
        }
    }
}

/// The matrix that changes nothing: `[1 0 0 1 0 0]`.
pub const IDENTITY_MATRIX: [f32; 6] = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

fn identity_matrix() -> [f32; 6] {
    IDENTITY_MATRIX
}

/// A pattern: a color that is a repeated drawing or a gradient. See the [module
/// documentation](crate::pattern).
///
/// Register one with [`PdfDocument::add_pattern`](crate::PdfDocument::add_pattern) and select it
/// with [`Op::SetFillPattern`] or [`Op::SetOutlinePattern`].
///
/// Serialized as `{"type": "tiling" | "shading" | "external", "data": ...}`.
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "type", content = "data")]
pub enum Pattern {
    /// A tiling pattern (`/PatternType 1`, ISO 32000-1, 8.7.3): `ops` draw one cell, which is
    /// repeated every `xstep` and `ystep` units.
    Tiling {
        /// Whether the ops set the colors, or the op that selects the pattern does.
        #[serde(default)]
        paint_type: PatternPaintType,
        /// `/BBox`: the area of one cell, in pattern space. The ops are clipped to it.
        bbox: Rect,
        /// `/XStep`: the horizontal distance between the start of two cells, in pattern space.
        /// It is not zero. When it is smaller than the width of `bbox` the cells overlap, and
        /// when it is larger there are gaps.
        xstep: f32,
        /// `/YStep`: the vertical distance between the start of two cells.
        ystep: f32,
        /// `/Matrix`: `[a b c d e f]`, from pattern space to the default coordinate system of
        /// the page or form that uses the pattern. The identity when it is not set in JSON.
        #[serde(default = "identity_matrix")]
        matrix: [f32; 6],
        /// The drawing of one cell. Names in the ops are looked up in
        /// [`resources`](Pattern::Tiling::resources).
        #[serde(default)]
        ops: Vec<Op>,
        /// `/Resources`: the fonts, graphics states, shadings and patterns the ops name. They
        /// are the pattern's own, as for a [`FormXObject`](crate::FormXObject). With `None` the
        /// ops name no resources, except the 14 standard fonts, which need none. A pattern does
        /// not use the resources of the page that paints it.
        #[serde(default)]
        resources: Option<PdfResources>,
    },
    /// A shading pattern (`/PatternType 2`, ISO 32000-1, 8.7.4): a gradient that fills the
    /// painted shape.
    Shading {
        /// The gradient. Its coordinates are in pattern space.
        shading: Shading,
        /// `/Matrix`: `[a b c d e f]`, from pattern space to the default coordinate system of
        /// the page or form that uses the pattern.
        #[serde(default = "identity_matrix")]
        matrix: [f32; 6],
        /// `/ExtGState`: graphics state parameters (such as the opacity) in effect while the
        /// pattern paints, in place of the current ones.
        #[serde(default)]
        ext_gstate: Option<ExtendedGraphicsState>,
    },
    /// A pattern that has no model, kept as it was read from a PDF: its dictionary, or its
    /// dictionary and stream, with the references resolved. Mesh shadings and tiling patterns
    /// with nested resources are kept this way. It is written as it is.
    External(DictItem),
}

impl Pattern {
    /// A colored tiling pattern: `ops` draw one cell inside `bbox`, repeated every `xstep` and
    /// `ystep`. The matrix is the identity and the pattern has no resources.
    pub fn colored_tiling(bbox: Rect, xstep: f32, ystep: f32, ops: Vec<Op>) -> Self {
        Pattern::Tiling {
            paint_type: PatternPaintType::Colored,
            bbox,
            xstep,
            ystep,
            matrix: IDENTITY_MATRIX,
            ops,
            resources: None,
        }
    }

    /// An uncolored tiling pattern: `ops` draw the shape of one cell without setting a color.
    /// The color comes from the op that selects the pattern.
    pub fn uncolored_tiling(bbox: Rect, xstep: f32, ystep: f32, ops: Vec<Op>) -> Self {
        Pattern::Tiling {
            paint_type: PatternPaintType::Uncolored,
            bbox,
            xstep,
            ystep,
            matrix: IDENTITY_MATRIX,
            ops,
            resources: None,
        }
    }

    /// A shading pattern for `shading`, with the identity matrix.
    pub fn from_shading(shading: Shading) -> Self {
        Pattern::Shading {
            shading,
            matrix: IDENTITY_MATRIX,
            ext_gstate: None,
        }
    }

    /// Sets the matrix of a tiling or shading pattern and returns the pattern. An
    /// [`Pattern::External`] is returned unchanged.
    pub fn with_matrix(mut self, new_matrix: [f32; 6]) -> Self {
        match &mut self {
            Pattern::Tiling { matrix, .. } | Pattern::Shading { matrix, .. } => {
                *matrix = new_matrix;
            }
            Pattern::External(_) => {}
        }
        self
    }

    /// Sets the resources of a tiling pattern and returns the pattern. Other patterns are
    /// returned unchanged.
    pub fn with_resources(mut self, new_resources: PdfResources) -> Self {
        if let Pattern::Tiling { resources, .. } = &mut self {
            *resources = Some(new_resources);
        }
        self
    }

    /// A single color that stands for the pattern, for output that cannot draw patterns, such as
    /// the SVG preview. `underlying` is the color of the op that selects the pattern.
    ///
    /// - A shading pattern gives its first color stop.
    /// - An uncolored tiling pattern gives `underlying`, or mid grey without one.
    /// - A colored tiling pattern gives the mean of the fill and stroke colors its ops set, or
    ///   mid grey when it sets none.
    /// - [`Pattern::External`] gives mid grey.
    pub fn approximate_color(&self, underlying: Option<&Color>) -> Color {
        let grey = Color::Rgb(crate::Rgb::new(0.5, 0.5, 0.5, None));
        match self {
            Pattern::Shading { shading, .. } => match shading.stops.first() {
                Some(stop) => {
                    Color::Rgb(crate::Rgb::new(stop.color[0], stop.color[1], stop.color[2], None))
                }
                None => grey,
            },
            Pattern::Tiling {
                paint_type: PatternPaintType::Uncolored,
                ..
            } => underlying.cloned().unwrap_or(grey),
            Pattern::Tiling { ops, .. } => {
                let mut sum = [0.0f32; 3];
                let mut count = 0.0f32;
                for op in ops {
                    if let Op::SetFillColor { col } | Op::SetOutlineColor { col } = op {
                        let rgb = color_to_rgb(col);
                        for (total, part) in sum.iter_mut().zip(rgb) {
                            *total += part;
                        }
                        count += 1.0;
                    }
                }
                if count == 0.0 {
                    grey
                } else {
                    Color::Rgb(crate::Rgb::new(
                        sum[0] / count,
                        sum[1] / count,
                        sum[2] / count,
                        None,
                    ))
                }
            }
            Pattern::External(_) => grey,
        }
    }
}

/// The color as `[r, g, b]`. CMYK uses the plain formula `(1 - c) * (1 - k)`, without a profile.
fn color_to_rgb(color: &Color) -> [f32; 3] {
    let clamp = |v: f32| if v.is_finite() { v.clamp(0.0, 1.0) } else { 0.0 };
    match color {
        Color::Rgb(c) => [clamp(c.r), clamp(c.g), clamp(c.b)],
        Color::Greyscale(g) => {
            let v = clamp(g.percent);
            [v, v, v]
        }
        Color::Cmyk(c) => cmyk_to_rgb(c.c, c.m, c.y, c.k),
        Color::SpotColor(c) => cmyk_to_rgb(c.c, c.m, c.y, c.k),
    }
}

fn cmyk_to_rgb(c: f32, m: f32, y: f32, k: f32) -> [f32; 3] {
    let clamp = |v: f32| if v.is_finite() { v.clamp(0.0, 1.0) } else { 0.0 };
    let k = 1.0 - clamp(k);
    [
        (1.0 - clamp(c)) * k,
        (1.0 - clamp(m)) * k,
        (1.0 - clamp(y)) * k,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{GradientStop, Greyscale, Pt, Rgb, ShadingGeometry};

    fn cell() -> Rect {
        Rect::from_wh(Pt(10.0), Pt(10.0))
    }

    fn gradient() -> Shading {
        Shading {
            geometry: ShadingGeometry::Axial { coords: [0.0, 0.0, 100.0, 0.0] },
            stops: vec![
                GradientStop { offset: 0.0, color: [1.0, 0.0, 0.0] },
                GradientStop { offset: 1.0, color: [0.0, 0.0, 1.0] },
            ],
            extend: (true, true),
        }
    }

    #[test]
    fn constructors_set_the_paint_type() {
        let colored = Pattern::colored_tiling(cell(), 10.0, 10.0, Vec::new());
        let uncolored = Pattern::uncolored_tiling(cell(), 10.0, 10.0, Vec::new());
        assert!(matches!(
            colored,
            Pattern::Tiling { paint_type: PatternPaintType::Colored, .. }
        ));
        assert!(matches!(
            uncolored,
            Pattern::Tiling { paint_type: PatternPaintType::Uncolored, .. }
        ));
    }

    #[test]
    fn builders_set_matrix_and_resources() {
        let matrix = [2.0, 0.0, 0.0, 2.0, 5.0, 5.0];
        let p = Pattern::colored_tiling(cell(), 10.0, 10.0, Vec::new())
            .with_matrix(matrix)
            .with_resources(PdfResources::default());
        match p {
            Pattern::Tiling { matrix: m, resources, .. } => {
                assert_eq!(m, matrix);
                assert!(resources.is_some());
            }
            other => panic!("not a tiling pattern: {other:?}"),
        }
        match Pattern::from_shading(gradient()).with_matrix(matrix) {
            Pattern::Shading { matrix: m, .. } => assert_eq!(m, matrix),
            other => panic!("not a shading pattern: {other:?}"),
        }
        // An external pattern has no matrix of its own.
        let external = Pattern::External(DictItem::Null);
        assert_eq!(external.clone().with_matrix(matrix), external);
    }

    #[test]
    fn pattern_ids_are_distinct() {
        assert_ne!(PatternId::new(), PatternId::new());
    }

    #[test]
    fn json_round_trip_and_defaults() {
        let p = Pattern::uncolored_tiling(cell(), 12.0, 8.0, vec![Op::SaveGraphicsState])
            .with_matrix([1.0, 0.0, 0.0, 1.0, 3.0, 4.0]);
        let json = serde_json::to_string(&p).expect("serialize");
        assert!(json.contains("\"type\":\"tiling\""), "{json}");
        let back: Pattern = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, p);

        // A tiling pattern without matrix, ops, paint type and resources gets the defaults.
        let json = r#"{"type":"tiling","data":{
            "bbox":{"x":0,"y":0,"width":4,"height":4},
            "xstep":4,"ystep":4}}"#;
        let p: Pattern = serde_json::from_str(json).expect("defaults");
        match p {
            Pattern::Tiling { paint_type, matrix, ops, resources, .. } => {
                assert_eq!(paint_type, PatternPaintType::Colored);
                assert_eq!(matrix, IDENTITY_MATRIX);
                assert!(ops.is_empty());
                assert!(resources.is_none());
            }
            other => panic!("not a tiling pattern: {other:?}"),
        }
    }

    #[test]
    fn old_resources_json_without_patterns_still_loads() {
        let json = r#"{"fonts":{},"xobjects":{},"extgstates":{},"shadings":{},"layers":{}}"#;
        let resources: PdfResources = serde_json::from_str(json).expect("old json");
        assert!(resources.patterns.map.is_empty());
    }

    #[test]
    fn ops_serialize_with_and_without_color() {
        let op = Op::SetFillPattern { id: PatternId("P1".into()), color: None };
        let json = serde_json::to_string(&op).expect("serialize");
        assert!(json.contains("set-fill-pattern"), "{json}");
        let back: Op = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, op);

        // `color` can be left out.
        let back: Op = serde_json::from_str(
            r#"{"type":"set-outline-pattern","data":{"id":"P2"}}"#,
        )
        .expect("no color");
        assert_eq!(back, Op::SetOutlinePattern { id: PatternId("P2".into()), color: None });
    }

    #[test]
    fn add_pattern_registers_it() {
        let mut doc = crate::PdfDocument::new("p");
        let id = doc.add_pattern(Pattern::from_shading(gradient()));
        assert!(doc.resources.patterns.map.contains_key(&id));
    }

    #[test]
    fn approximate_colors() {
        let red = Color::Rgb(Rgb::new(1.0, 0.0, 0.0, None));
        // A shading pattern: the first stop.
        assert_eq!(Pattern::from_shading(gradient()).approximate_color(None), red);
        // An uncolored pattern: the color of the op, or grey.
        let uncolored = Pattern::uncolored_tiling(cell(), 4.0, 4.0, Vec::new());
        assert_eq!(uncolored.approximate_color(Some(&red)), red);
        assert_eq!(
            uncolored.approximate_color(None),
            Color::Rgb(Rgb::new(0.5, 0.5, 0.5, None))
        );
        // A colored pattern: the mean of the colors it sets.
        let ops = vec![
            Op::SetFillColor { col: red.clone() },
            Op::SetOutlineColor { col: Color::Greyscale(Greyscale::new(1.0, None)) },
        ];
        let colored = Pattern::colored_tiling(cell(), 4.0, 4.0, ops);
        assert_eq!(
            colored.approximate_color(None),
            Color::Rgb(Rgb::new(1.0, 0.5, 0.5, None))
        );
        // No colors set and an external pattern: grey.
        let none = Pattern::colored_tiling(cell(), 4.0, 4.0, Vec::new());
        assert_eq!(none.approximate_color(None), Color::Rgb(Rgb::new(0.5, 0.5, 0.5, None)));
        assert_eq!(
            Pattern::External(DictItem::Null).approximate_color(None),
            Color::Rgb(Rgb::new(0.5, 0.5, 0.5, None))
        );
    }
}
