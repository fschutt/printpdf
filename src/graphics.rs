use std::collections::HashSet;

use lopdf::Dictionary as LoDictionary;
use serde_derive::{Deserialize, Serialize};

use crate::{
    FontId,
    units::{Mm, Pt},
};

/// Fill path using nonzero winding number rule
pub const OP_PATH_PAINT_FILL_NZ: &str = "f";
/// Fill path using even-odd rule
pub const OP_PATH_PAINT_FILL_EO: &str = "f*";
/// Fill and stroke path using nonzero winding number rule
pub const OP_PATH_PAINT_FILL_STROKE_NZ: &str = "B";
/// Close, fill and stroke path using nonzero winding number rule
pub const OP_PATH_PAINT_FILL_STROKE_CLOSE_NZ: &str = "b";
/// Fill and stroke path using even-odd rule
pub const OP_PATH_PAINT_FILL_STROKE_EO: &str = "B*";
/// Close, fill and stroke path using even odd rule
pub const OP_PATH_PAINT_FILL_STROKE_CLOSE_EO: &str = "b*";
/// Current path is a clip path, non-zero winding order (usually in like `h W S`)
pub const OP_PATH_CONST_CLIP_NZ: &str = "W";
/// Current path is a clip path, non-zero winding order
pub const OP_PATH_CONST_CLIP_EO: &str = "W*";

/// Rectangle struct (x, y, width, height) from the LOWER LEFT corner of the page
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rect {
    pub x: Pt,
    pub y: Pt,
    pub width: Pt,
    pub height: Pt,
}

impl Rect {
    pub fn lower_left(&self) -> Point {
        Point {
            x: self.x,
            y: self.y,
        }
    }

    pub fn upper_right(&self) -> Point {
        Point {
            x: self.x + self.width,
            y: self.y + self.height,
        }
    }

    pub fn from_wh(width: Pt, height: Pt) -> Self {
        Self {
            x: Pt(0.0),
            y: Pt(0.0),
            width,
            height,
        }
    }

    pub fn to_polygon(&self) -> Polygon {
        Polygon {
            rings: vec![PolygonRing {
                points: self.gen_points(),
            }],
            mode: PaintMode::Fill,
            winding_order: WindingOrder::NonZero,
        }
    }

    pub fn to_line(&self) -> Line {
        Line {
            points: self.gen_points(),
            is_closed: true,
        }
    }

    fn gen_points(&self) -> Vec<LinePoint> {
        let top = self.y;
        let bottom = Pt(self.y.0 - self.height.0);
        let left = self.x;
        let right = Pt(self.x.0 + self.width.0);

        let tl = Point { x: left, y: top };
        let tr = Point { x: right, y: top };
        let br = Point {
            x: right,
            y: bottom,
        };
        let bl = Point { x: left, y: bottom };

        vec![
            LinePoint {
                p: tl,
                bezier: false,
            },
            LinePoint {
                p: tr,
                bezier: false,
            },
            LinePoint {
                p: br,
                bezier: false,
            },
            LinePoint {
                p: bl,
                bezier: false,
            },
        ]
    }

    pub fn to_array(&self) -> Vec<lopdf::Object> {
        vec![
            (self.x.0.round() as i64).into(),
            (self.y.0.round() as i64).into(),
            (self.width.0.round() as i64).into(),
            (self.height.0.round() as i64).into(),
        ]
    }
}

/// The rule to use in filling/clipping paint operations.
///
/// This is meaningful in the following cases:
///
/// - When a path uses one of the _fill_ paint operations, this will determine the rule used to fill
///   the paths.
/// - When a path uses a [clip] painting mode, this will determine the rule used to limit the
///   regions of the page affected by painting operators.
///
/// Most of the time, `NonZero` is the appropriate option.
///
/// [clip]: PaintMode::Clip
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WindingOrder {
    /// Make any filling or clipping paint operators follow the _even-odd rule_.
    ///
    /// This rule determines whether a point is inside a path by drawing a ray from that point in
    /// any direction and simply counting the number of path segments that cross the ray,
    /// regardless of direction. If this number is odd, the point is inside; if even, the point is
    /// outside. This yields the same results as the nonzero winding number rule for paths with
    /// simple shapes, but produces different results for more complex shapes.
    EvenOdd,

    /// Make any filling or clipping paint operators follow the _nonzero rule_.
    ///
    /// This rule determines whether a given point is inside a path by conceptually drawing a ray
    /// from that point to infinity in any direction and then examining the places where a segment
    /// of the path crosses the ray. Starting with a count of 0, the rule adds 1 each time a path
    /// segment crosses the ray from left to right and subtracts 1 each time a segment crosses from
    /// right to left. After counting all the crossings, if the result is 0, the point is outside
    /// the path; otherwise, it is inside.
    #[default]
    NonZero,
}

impl WindingOrder {
    /// Gets the operator for a clip paint operation.
    #[must_use]
    pub fn get_clip_op(&self) -> &'static str {
        match self {
            WindingOrder::NonZero => OP_PATH_CONST_CLIP_NZ,
            WindingOrder::EvenOdd => OP_PATH_CONST_CLIP_EO,
        }
    }

    /// Gets the operator for a fill paint operation.
    #[must_use]
    pub fn get_fill_op(&self) -> &'static str {
        match self {
            WindingOrder::NonZero => OP_PATH_PAINT_FILL_NZ,
            WindingOrder::EvenOdd => OP_PATH_PAINT_FILL_EO,
        }
    }

    /// Gets the operator for a close, fill and stroke painting operation.
    #[must_use]
    pub fn get_fill_stroke_close_op(&self) -> &'static str {
        match self {
            WindingOrder::NonZero => OP_PATH_PAINT_FILL_STROKE_CLOSE_NZ,
            WindingOrder::EvenOdd => OP_PATH_PAINT_FILL_STROKE_CLOSE_EO,
        }
    }

    /// Gets the operator for a fill and stroke painting operation.
    #[must_use]
    pub fn get_fill_stroke_op(&self) -> &'static str {
        match self {
            WindingOrder::NonZero => OP_PATH_PAINT_FILL_STROKE_NZ,
            WindingOrder::EvenOdd => OP_PATH_PAINT_FILL_STROKE_EO,
        }
    }
}

/// The path-painting mode for a path.
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PaintMode {
    /// Set the path in clipping mode instead of painting it.
    ///
    /// The path is not being drawing, but it will be used for clipping operations instead. The
    /// rule for clipping are determined by the value [`WindingOrder`] associated to the path.
    Clip,

    /// Fill the path.
    #[default]
    Fill,

    /// Paint a line along the path.
    Stroke,

    /// Fill the path and paint a line along it.
    FillStroke,
}

#[derive(Debug, Default, Copy, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Point {
    /// x position from the bottom left corner in pt
    pub x: Pt,
    /// y position from the bottom left corner in pt
    pub y: Pt,
}

impl Point {
    /// Create a new point.
    /// **WARNING: The reference point for a point is the bottom left corner, not the top left**
    #[inline]
    pub fn new(x: Mm, y: Mm) -> Self {
        Self {
            x: x.into(),
            y: y.into(),
        }
    }
}

impl PartialEq for Point {
    // custom compare function because of floating point inaccuracy
    fn eq(&self, other: &Point) -> bool {
        if self.x.0.is_normal()
            && other.x.0.is_normal()
            && self.y.0.is_normal()
            && other.y.0.is_normal()
        {
            // four floating point numbers have to match
            let x_eq = self.x == other.x;
            if !x_eq {
                return false;
            }
            let y_eq = self.y == other.y;
            if y_eq {
                return true;
            }
        }

        false
    }
}

/// Either a point or a bezier control point
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinePoint {
    /// Location of the point
    pub p: Point,
    /// If `true`, this point is a bezier control point
    pub bezier: bool,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Line {
    /// 2D Points for the line
    pub points: Vec<LinePoint>,
    /// Whether the line should automatically be closed
    pub is_closed: bool,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Polygon {
    /// 2D Points for the line. The `bool` indicates whether the next point is a bezier control
    /// point.
    pub rings: Vec<PolygonRing>,
    /// What type of polygon is this?
    pub mode: PaintMode,
    /// Winding order to use for constructing this polygon
    pub winding_order: WindingOrder,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolygonRing {
    /// 2D Points for the ring
    pub points: Vec<LinePoint>,
}

impl FromIterator<(Point, bool)> for Polygon {
    fn from_iter<I: IntoIterator<Item = (Point, bool)>>(iter: I) -> Self {
        let mut points = Vec::new();
        for i in iter {
            points.push(LinePoint {
                p: i.0,
                bezier: i.1,
            });
        }
        Polygon {
            rings: vec![PolygonRing { points }],
            ..Default::default()
        }
    }
}

/// Line dash pattern is made up of a total width
#[derive(Debug, Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LineDashPattern {
    /// Offset at which the dashing pattern should start, measured from the beginning ot the line
    /// Default: 0 (start directly where the line starts)
    pub offset: i64,
    /// Length of the first dash in the dash pattern. If `None`, the line will be solid (good for
    /// resetting the dash pattern)
    #[serde(default)]
    pub dash_1: Option<i64>,
    /// Whitespace after the first dash. If `None`, whitespace will be the same as length_1st,
    /// meaning that the line will have dash - whitespace - dash - whitespace in even offsets
    #[serde(default)]
    pub gap_1: Option<i64>,
    /// Length of the second dash in the dash pattern. If None, will be equal to length_1st
    #[serde(default)]
    pub dash_2: Option<i64>,
    /// Same as whitespace_1st, but for length_2nd
    #[serde(default)]
    pub gap_2: Option<i64>,
    /// Length of the second dash in the dash pattern. If None, will be equal to length_1st
    #[serde(default)]
    pub dash_3: Option<i64>,
    /// Same as whitespace_1st, but for length_3rd
    #[serde(default)]
    pub gap_3: Option<i64>,
}

impl LineDashPattern {
    pub fn as_array(&self) -> Vec<i64> {
        [
            self.dash_1,
            self.gap_1,
            self.dash_2,
            self.gap_2,
            self.dash_3,
            self.gap_3,
        ]
        .iter()
        .copied()
        .take_while(Option::is_some)
        .flatten()
        .collect()
    }

    pub fn get_svg_id(&self) -> String {
        let dash_array = self.as_array();
        dash_array
            .iter()
            .map(|num| num.to_string())
            .collect::<Vec<_>>()
            .join(",")
    }

    /// Builds a `LineDashPattern` from a slice of up to 6 integers.
    ///
    /// - The array is interpreted in dash-gap pairs:
    ///   - If `dashes[0]` is present => `dash_1 = Some(...)`
    ///   - If `dashes[1]` is present => `gap_1 = Some(...)`
    ///   - If `dashes[2]` is present => `dash_2 = Some(...)`
    ///   - If `dashes[3]` is present => `gap_2 = Some(...)`
    ///   - If `dashes[4]` is present => `dash_3 = Some(...)`
    ///   - If `dashes[5]` is present => `gap_3 = Some(...)`
    ///
    /// Any extra elements beyond index 5 are ignored. If the slice is empty,
    /// the line is solid (all fields `None`).
    pub fn from_array(dashes: &[i64], offset: i64) -> Self {
        let mut pat = LineDashPattern::default();
        pat.offset = offset;

        match dashes.len() {
            0 => {
                // No dashes => solid line
                // (everything is None, which is already default)
            }
            1 => {
                pat.dash_1 = Some(dashes[0]);
            }
            2 => {
                pat.dash_1 = Some(dashes[0]);
                pat.gap_1 = Some(dashes[1]);
            }
            3 => {
                pat.dash_1 = Some(dashes[0]);
                pat.gap_1 = Some(dashes[1]);
                pat.dash_2 = Some(dashes[2]);
            }
            4 => {
                pat.dash_1 = Some(dashes[0]);
                pat.gap_1 = Some(dashes[1]);
                pat.dash_2 = Some(dashes[2]);
                pat.gap_2 = Some(dashes[3]);
            }
            5 => {
                pat.dash_1 = Some(dashes[0]);
                pat.gap_1 = Some(dashes[1]);
                pat.dash_2 = Some(dashes[2]);
                pat.gap_2 = Some(dashes[3]);
                pat.dash_3 = Some(dashes[4]);
            }
            _ => {
                // 6 or more elements => fill all 3 dash-gap pairs
                pat.dash_1 = Some(dashes[0]);
                pat.gap_1 = Some(dashes[1]);
                pat.dash_2 = Some(dashes[2]);
                pat.gap_2 = Some(dashes[3]);
                pat.dash_3 = Some(dashes[4]);
                pat.gap_3 = Some(dashes[5]);
            }
        }

        pat
    }
}

/// __See PDF Reference Page 216__ - Line join style
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LineJoinStyle {
    /// Miter join. The outer edges of the strokes for the two segments are extended
    /// until they meet at an angle, as in a picture frame. If the segments meet at too
    /// sharp an angle (as defined by the miter limit parameter—see “Miter Limit,”
    /// above), a bevel join is used instead.
    Miter,
    /// Round join. An arc of a circle with a diameter equal to the line width is drawn
    /// around the point where the two segments meet, connecting the outer edges of
    /// the strokes for the two segments. This pieslice-shaped figure is filled in, pro-
    /// ducing a rounded corner.
    Round,
    /// Bevel join. The two segments are finished with butt caps (see “Line Cap Style”
    /// on page 216) and the resulting notch beyond the ends of the segments is filled
    /// with a triangle.
    Bevel,
}

impl LineJoinStyle {
    pub fn id(&self) -> i64 {
        match self {
            LineJoinStyle::Miter => 0,
            LineJoinStyle::Round => 1,
            LineJoinStyle::Bevel => 2,
        }
    }
    pub fn to_svg_string(&self) -> &'static str {
        match self {
            LineJoinStyle::Miter => "miter",
            LineJoinStyle::Round => "round",
            LineJoinStyle::Bevel => "bevel",
        }
    }
}

/// The text rendering mode determines how a text is drawn
/// The default rendering mode is `Fill`. The color of the
/// fill / stroke is determine by the current pages outline /
/// fill color.
///
/// See PDF Reference 1.7 Page 402
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TextRenderingMode {
    Fill,
    Stroke,
    FillStroke,
    Invisible,
    FillClip,
    StrokeClip,
    FillStrokeClip,
    Clip,
}

impl TextRenderingMode {
    pub fn from_i64(i: i64) -> Self {
        match i {
            0 => TextRenderingMode::Fill,
            1 => TextRenderingMode::Stroke,
            2 => TextRenderingMode::FillStroke,
            3 => TextRenderingMode::Invisible,
            4 => TextRenderingMode::FillClip,
            5 => TextRenderingMode::StrokeClip,
            6 => TextRenderingMode::FillStrokeClip,
            7 => TextRenderingMode::Clip,
            _ => TextRenderingMode::Fill,
        }
    }
    pub fn id(&self) -> i64 {
        match self {
            TextRenderingMode::Fill => 0,
            TextRenderingMode::Stroke => 1,
            TextRenderingMode::FillStroke => 2,
            TextRenderingMode::Invisible => 3,
            TextRenderingMode::FillClip => 4,
            TextRenderingMode::StrokeClip => 5,
            TextRenderingMode::FillStrokeClip => 6,
            TextRenderingMode::Clip => 7,
        }
    }
}

/// __See PDF Reference (Page 216)__ - Line cap (ending) style
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LineCapStyle {
    /// Butt cap. The stroke is squared off at the endpoint of the path. There is no
    /// projection beyond the end of the path.
    Butt,
    /// Round cap. A semicircular arc with a diameter equal to the line width is
    /// drawn around the endpoint and filled in.
    Round,
    /// Projecting square cap. The stroke continues beyond the endpoint of the path
    /// for a distance equal to half the line width and is squared off.
    ProjectingSquare,
}

impl LineCapStyle {
    pub fn id(&self) -> i64 {
        match self {
            LineCapStyle::Butt => 0,
            LineCapStyle::Round => 1,
            LineCapStyle::ProjectingSquare => 2,
        }
    }

    pub fn get_svg_id(&self) -> &'static str {
        match self {
            LineCapStyle::Butt => "butt",
            LineCapStyle::Round => "round",
            LineCapStyle::ProjectingSquare => "square",
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename = "kebap-case")]
pub enum ChangedField {
    LineWidth,
    LineCap,
    LineJoin,
    MiterLimit,
    LineDashPattern,
    RenderingIntent,
    OverprintStroke,
    OverprintFill,
    OverprintMode,
    Font,
    BlackGeneration,
    BlackGenerationExtra,
    UnderColorRemoval,
    UnderColorRemovalExtra,
    TransferFunction,
    TransferFunctionExtra,
    HalftoneDictionary,
    FlatnessTolerance,
    SmoothnessTolerance,
    StrokeAdjustment,
    BlendMode,
    SoftMask,
    CurrentStrokeAlpha,
    CurrentFillAlpha,
    AlphaIsShape,
    TextKnockout,
}

/// `ExtGState` dictionary
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtendedGraphicsState {
    /// A set to track which fields have changed in relation to the default() method.
    /// Now using a strongly typed enum instead of string constants.
    pub(crate) changed_fields: HashSet<ChangedField>,

    /* LW float */
    /// __(Optional; PDF 1.3)__ The current line width
    pub(crate) line_width: f32,

    /* LC integer */
    /// __(Optional; PDF 1.3)__ The current line cap style
    pub(crate) line_cap: LineCapStyle,

    /* LJ integer */
    /// __(Optional; PDF 1.3)__ The current line join style
    pub(crate) line_join: LineJoinStyle,

    /* ML float */
    /// __(Optional; PDF 1.3)__ The miter limit (see “Miter Limit” on page 217).
    pub(crate) miter_limit: f32,

    /* D array */
    /// __(Optional; PDF 1.3)__ The line dash pattern.
    pub(crate) line_dash_pattern: Option<LineDashPattern>,

    /* RI name (or ri inside a stream) */
    /// __(Optional; PDF 1.3)__ The name of the rendering intent.
    pub(crate) rendering_intent: RenderingIntent,

    /* OP boolean */
    /// __(Optional)__ Overprint flag for stroking.
    pub(crate) overprint_stroke: bool,

    /* op boolean */
    /// __(Optional; PDF 1.3)__ Overprint flag for nonstroking.
    pub(crate) overprint_fill: bool,

    /* OPM integer */
    /// __(Optional; PDF 1.3)__ The overprint mode.
    pub(crate) overprint_mode: OverprintMode,

    /* Font array */
    /// Font structure, expects a dictionary.
    pub(crate) font: Option<FontId>,

    /* BG function */
    /// __(Optional)__ The black-generation function.
    pub(crate) black_generation: Option<BlackGenerationFunction>,

    /* BG2 function or name */
    /// __(Optional; PDF 1.3)__ The extra black-generation function.
    pub(crate) black_generation_extra: Option<BlackGenerationExtraFunction>,

    /* UCR function */
    /// __(Optional)__ The undercolor-removal function.
    pub(crate) under_color_removal: Option<UnderColorRemovalFunction>,

    /* UCR2 function */
    /// __(Optional; PDF 1.3)__ The extra undercolor-removal function.
    pub(crate) under_color_removal_extra: Option<UnderColorRemovalExtraFunction>,

    /* TR function */
    /// __(Optional)__ The transfer function.
    pub(crate) transfer_function: Option<TransferFunction>,

    /* TR2 function */
    /// __(Optional; PDF 1.3)__ The extra transfer function.
    pub(crate) transfer_extra_function: Option<TransferExtraFunction>,

    /* HT [dictionary, stream or name] */
    /// __(Optional)__ The halftone dictionary or stream.
    pub(crate) halftone_dictionary: Option<HalftoneType>,

    /* FL integer */
    /// __(Optional; PDF 1.3)__ The flatness tolerance.
    pub(crate) flatness_tolerance: f32,

    /* SM integer */
    /// __(Optional; PDF 1.3)__ The smoothness tolerance.
    pub(crate) smoothness_tolerance: f32,

    /* SA integer */
    /// (Optional) Automatic stroke adjustment flag.
    pub(crate) stroke_adjustment: bool,

    /* BM name or array */
    /// __(Optional; PDF 1.4)__ The blend mode.
    pub(crate) blend_mode: BlendMode,

    /* SM dictionary or name */
    /// __(Optional; PDF 1.4)__ The soft mask.
    pub(crate) soft_mask: Option<SoftMask>,

    /* CA integer */
    /// __(Optional; PDF 1.4)__ The current stroking alpha constant.
    pub(crate) current_stroke_alpha: f32,

    /* ca integer */
    /// __(Optional; PDF 1.4)__ The current nonstroking alpha constant.
    pub(crate) current_fill_alpha: f32,

    /* AIS boolean */
    /// __(Optional; PDF 1.4)__ The alpha source flag.
    pub(crate) alpha_is_shape: bool,

    /* TK boolean */
    /// __(Optional; PDF 1.4)__ The text knockout flag.
    pub(crate) text_knockout: bool,
}

pub fn extgstate_to_dict(val: &ExtendedGraphicsState) -> LoDictionary {
    use std::string::String;

    use lopdf::Object::*;

    let mut gs_operations = Vec::<(String, lopdf::Object)>::new();

    if val.changed_fields.contains(&ChangedField::LineWidth) {
        gs_operations.push(("LW".to_string(), Real(val.line_width)));
    }

    if val.changed_fields.contains(&ChangedField::LineCap) {
        gs_operations.push(("LC".to_string(), Integer(val.line_cap.id())));
    }

    if val.changed_fields.contains(&ChangedField::LineJoin) {
        gs_operations.push(("LJ".to_string(), Integer(val.line_join.id())));
    }

    if val.changed_fields.contains(&ChangedField::MiterLimit) {
        gs_operations.push(("ML".to_string(), Real(val.miter_limit)));
    }

    if val
        .changed_fields
        .contains(&ChangedField::FlatnessTolerance)
    {
        gs_operations.push(("FL".to_string(), Real(val.flatness_tolerance)));
    }

    if val.changed_fields.contains(&ChangedField::RenderingIntent) {
        gs_operations.push(("RI".to_string(), Name(val.rendering_intent.get_id().into())));
    }

    if val.changed_fields.contains(&ChangedField::StrokeAdjustment) {
        gs_operations.push(("SA".to_string(), Boolean(val.stroke_adjustment)));
    }

    if val.changed_fields.contains(&ChangedField::OverprintFill) {
        gs_operations.push(("OP".to_string(), Boolean(val.overprint_fill)));
    }

    if val.changed_fields.contains(&ChangedField::OverprintStroke) {
        gs_operations.push(("op".to_string(), Boolean(val.overprint_stroke)));
    }

    if val.changed_fields.contains(&ChangedField::OverprintMode) {
        gs_operations.push(("OPM".to_string(), Integer(val.overprint_mode.get_id())));
    }

    if val.changed_fields.contains(&ChangedField::CurrentFillAlpha) {
        gs_operations.push(("CA".to_string(), Real(val.current_fill_alpha)));
    }

    if val
        .changed_fields
        .contains(&ChangedField::CurrentStrokeAlpha)
    {
        gs_operations.push(("ca".to_string(), Real(val.current_stroke_alpha)));
    }

    if val.changed_fields.contains(&ChangedField::BlendMode) {
        gs_operations.push(("BM".to_string(), Name(val.blend_mode.get_id().into())));
    }

    if val.changed_fields.contains(&ChangedField::AlphaIsShape) {
        gs_operations.push(("AIS".to_string(), Boolean(val.alpha_is_shape)));
    }

    if val.changed_fields.contains(&ChangedField::TextKnockout) {
        gs_operations.push(("TK".to_string(), Boolean(val.text_knockout)));
    }

    // Optional parameters
    if let Some(ldp) = val.line_dash_pattern {
        if val.changed_fields.contains(&ChangedField::LineDashPattern) {
            let array = ldp.as_array().into_iter().map(Integer).collect();
            gs_operations.push(("D".to_string(), Array(array)));
        }
    }

    if let Some(font) = val.font.as_ref() {
        if val.changed_fields.contains(&ChangedField::Font) {
            gs_operations.push(("Font".to_string(), Name(font.0.clone().into_bytes())));
        }
    }

    // TODO: Handle transfer functions, halftone dictionary, black generation, etc.
    if val.changed_fields.contains(&ChangedField::BlackGeneration) {
        if let Some(ref _black_generation) = val.black_generation {
            // TODO
        }
    }

    if val
        .changed_fields
        .contains(&ChangedField::BlackGenerationExtra)
    {
        if let Some(ref _black_generation_extra) = val.black_generation_extra {
            // TODO
        }
    }

    if val
        .changed_fields
        .contains(&ChangedField::UnderColorRemoval)
    {
        if let Some(ref _under_color_removal) = val.under_color_removal {
            // TODO
        }
    }

    if val
        .changed_fields
        .contains(&ChangedField::UnderColorRemovalExtra)
    {
        if let Some(ref _under_color_removal_extra) = val.under_color_removal_extra {
            // TODO
        }
    }

    if val.changed_fields.contains(&ChangedField::TransferFunction) {
        if let Some(ref _transfer_function) = val.transfer_function {
            // TODO
        }
    }

    if val
        .changed_fields
        .contains(&ChangedField::TransferFunctionExtra)
    {
        if let Some(ref _transfer_extra_function) = val.transfer_extra_function {
            // TODO
        }
    }

    if val
        .changed_fields
        .contains(&ChangedField::HalftoneDictionary)
    {
        if let Some(ref _halftone_dictionary) = val.halftone_dictionary {
            // TODO
        }
    }

    if val.changed_fields.contains(&ChangedField::SoftMask) {
        if let Some(ref _soft_mask) = val.soft_mask {
            // Soft mask conversion can be handled here.
        } else {
            gs_operations.push(("SM".to_string(), Name("None".as_bytes().to_vec())));
        }
    }

    // If there are any operations, add the "Type" key
    if !gs_operations.is_empty() {
        gs_operations.push(("Type".to_string(), "ExtGState".into()));
    }

    LoDictionary::from_iter(gs_operations)
}

#[derive(Debug, Clone, Default)]
pub struct ExtendedGraphicsStateBuilder {
    /// Private field so we can control the `changed_fields` parameter
    gs: ExtendedGraphicsState,
}

impl ExtendedGraphicsStateBuilder {
    /// Creates a new graphics state builder
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the line width
    #[inline]
    pub fn with_line_width(mut self, line_width: f32) -> Self {
        self.gs.line_width = line_width;
        self.gs.changed_fields.insert(ChangedField::LineWidth);
        self
    }

    /// Sets the line cap
    #[inline]
    pub fn with_line_cap(mut self, line_cap: LineCapStyle) -> Self {
        self.gs.line_cap = line_cap;
        self.gs.changed_fields.insert(ChangedField::LineCap);
        self
    }

    /// Sets the line join
    #[inline]
    pub fn with_line_join(mut self, line_join: LineJoinStyle) -> Self {
        self.gs.line_join = line_join;
        self.gs.changed_fields.insert(ChangedField::LineJoin);
        self
    }

    /// Sets the miter limit
    #[inline]
    pub fn with_miter_limit(mut self, miter_limit: f32) -> Self {
        self.gs.miter_limit = miter_limit;
        self.gs.changed_fields.insert(ChangedField::MiterLimit);
        self
    }

    /// Sets the rendering intent
    #[inline]
    pub fn with_rendering_intent(mut self, rendering_intent: RenderingIntent) -> Self {
        self.gs.rendering_intent = rendering_intent;
        self.gs.changed_fields.insert(ChangedField::RenderingIntent);
        self
    }

    /// Sets the stroke overprint
    #[inline]
    pub fn with_overprint_stroke(mut self, overprint_stroke: bool) -> Self {
        self.gs.overprint_stroke = overprint_stroke;
        self.gs.changed_fields.insert(ChangedField::OverprintStroke);
        self
    }

    /// Sets the fill overprint
    #[inline]
    pub fn with_overprint_fill(mut self, overprint_fill: bool) -> Self {
        self.gs.overprint_fill = overprint_fill;
        self.gs.changed_fields.insert(ChangedField::OverprintFill);
        self
    }

    /// Sets the overprint mode
    #[inline]
    pub fn with_overprint_mode(mut self, overprint_mode: OverprintMode) -> Self {
        self.gs.overprint_mode = overprint_mode;
        self.gs.changed_fields.insert(ChangedField::OverprintMode);
        self
    }

    /// Sets the font
    /// __WARNING:__ Use `layer.add_font()` instead if you are not absolutely sure.
    #[inline]
    pub fn with_font(mut self, font: Option<FontId>) -> Self {
        self.gs.font = font;
        self.gs.changed_fields.insert(ChangedField::Font);
        self
    }

    /// Sets the black generation
    #[inline]
    pub fn with_black_generation(
        mut self,
        black_generation: Option<BlackGenerationFunction>,
    ) -> Self {
        self.gs.black_generation = black_generation;
        self.gs.changed_fields.insert(ChangedField::BlackGeneration);
        self
    }

    /// Sets the black generation extra function
    #[inline]
    pub fn with_black_generation_extra(
        mut self,
        black_generation_extra: Option<BlackGenerationExtraFunction>,
    ) -> Self {
        self.gs.black_generation_extra = black_generation_extra;
        self.gs
            .changed_fields
            .insert(ChangedField::BlackGenerationExtra);
        self
    }

    /// Sets the undercolor removal function
    #[inline]
    pub fn with_undercolor_removal(
        mut self,
        under_color_removal: Option<UnderColorRemovalFunction>,
    ) -> Self {
        self.gs.under_color_removal = under_color_removal;
        self.gs
            .changed_fields
            .insert(ChangedField::UnderColorRemoval);
        self
    }

    /// Sets the undercolor removal extra function
    #[inline]
    pub fn with_undercolor_removal_extra(
        mut self,
        under_color_removal_extra: Option<UnderColorRemovalExtraFunction>,
    ) -> Self {
        self.gs.under_color_removal_extra = under_color_removal_extra;
        self.gs
            .changed_fields
            .insert(ChangedField::UnderColorRemovalExtra);
        self
    }

    /// Sets the transfer function
    #[inline]
    pub fn with_transfer(mut self, transfer_function: Option<TransferFunction>) -> Self {
        self.gs.transfer_function = transfer_function;
        self.gs
            .changed_fields
            .insert(ChangedField::TransferFunction);
        self
    }

    /// Sets the transfer extra function
    #[inline]
    pub fn with_transfer_extra(
        mut self,
        transfer_extra_function: Option<TransferExtraFunction>,
    ) -> Self {
        self.gs.transfer_extra_function = transfer_extra_function;
        self.gs
            .changed_fields
            .insert(ChangedField::TransferFunctionExtra);
        self
    }

    /// Sets the halftone dictionary
    #[inline]
    pub fn with_halftone(mut self, halftone_type: Option<HalftoneType>) -> Self {
        self.gs.halftone_dictionary = halftone_type;
        self.gs
            .changed_fields
            .insert(ChangedField::HalftoneDictionary);
        self
    }

    /// Sets the flatness tolerance
    #[inline]
    pub fn with_flatness_tolerance(mut self, flatness_tolerance: f32) -> Self {
        self.gs.flatness_tolerance = flatness_tolerance;
        self.gs
            .changed_fields
            .insert(ChangedField::FlatnessTolerance);
        self
    }

    /// Sets the smoothness tolerance
    #[inline]
    pub fn with_smoothness_tolerance(mut self, smoothness_tolerance: f32) -> Self {
        self.gs.smoothness_tolerance = smoothness_tolerance;
        self.gs
            .changed_fields
            .insert(ChangedField::SmoothnessTolerance);
        self
    }

    /// Sets the stroke adjustment
    #[inline]
    pub fn with_stroke_adjustment(mut self, stroke_adjustment: bool) -> Self {
        self.gs.stroke_adjustment = stroke_adjustment;
        self.gs
            .changed_fields
            .insert(ChangedField::StrokeAdjustment);
        self
    }

    /// Sets the blend mode
    #[inline]
    pub fn with_blend_mode(mut self, blend_mode: BlendMode) -> Self {
        self.gs.blend_mode = blend_mode;
        self.gs.changed_fields.insert(ChangedField::BlendMode);
        self
    }

    /// Sets the soft mask
    #[inline]
    pub fn with_soft_mask(mut self, soft_mask: Option<SoftMask>) -> Self {
        self.gs.soft_mask = soft_mask;
        self.gs.changed_fields.insert(ChangedField::SoftMask);
        self
    }

    /// Sets the current alpha for strokes
    #[inline]
    pub fn with_current_stroke_alpha(mut self, current_stroke_alpha: f32) -> Self {
        self.gs.current_stroke_alpha = current_stroke_alpha;
        self.gs
            .changed_fields
            .insert(ChangedField::CurrentStrokeAlpha);
        self
    }

    /// Sets the current alpha for fills
    #[inline]
    pub fn with_current_fill_alpha(mut self, current_fill_alpha: f32) -> Self {
        self.gs.current_fill_alpha = current_fill_alpha;
        self.gs
            .changed_fields
            .insert(ChangedField::CurrentFillAlpha);
        self
    }

    /// Sets the current "alpha is shape"
    #[inline]
    pub fn with_alpha_is_shape(mut self, alpha_is_shape: bool) -> Self {
        self.gs.alpha_is_shape = alpha_is_shape;
        self.gs.changed_fields.insert(ChangedField::AlphaIsShape);
        self
    }

    /// Sets the current text knockout
    #[inline]
    pub fn with_text_knockout(mut self, text_knockout: bool) -> Self {
        self.gs.text_knockout = text_knockout;
        self.gs.changed_fields.insert(ChangedField::TextKnockout);
        self
    }

    /// Consumes the builder and returns an actual ExtendedGraphicsState
    #[inline]
    pub fn build(self) -> ExtendedGraphicsState {
        self.gs
    }
}

impl Default for ExtendedGraphicsState {
    /// Creates a default ExtGState dictionary. Useful for resetting
    fn default() -> Self {
        Self {
            changed_fields: HashSet::new(),
            line_width: 1.0,
            line_cap: LineCapStyle::Butt,
            line_join: LineJoinStyle::Miter,
            miter_limit: 0.0,
            line_dash_pattern: None,
            rendering_intent: RenderingIntent::RelativeColorimetric,
            overprint_stroke: false,
            overprint_fill: false,
            overprint_mode: OverprintMode::EraseUnderlying,
            font: None,
            black_generation: None,
            black_generation_extra: None,
            under_color_removal: None,
            under_color_removal_extra: None,
            transfer_function: None,
            transfer_extra_function: None,
            halftone_dictionary: None,
            flatness_tolerance: 0.0,
            smoothness_tolerance: 0.0,
            stroke_adjustment: true,
            blend_mode: BlendMode::Seperable(SeperableBlendMode::Normal),
            soft_mask: None,
            current_stroke_alpha: 1.0, /* 1.0 = opaque, not transparent */
            current_fill_alpha: 1.0,
            alpha_is_shape: false,
            text_knockout: false,
        }
    }
}

/// __(PDF 1.3)__ A code specifying whether a color component value of 0
/// in a `DeviceCMYK` color space should erase that component (`EraseUnderlying`) or
/// leave it unchanged (`KeepUnderlying`) when overprinting (see Section 4.5.6, “Over-
/// print Control”). Initial value: `EraseUnderlying`
#[derive(Debug, PartialEq, Copy, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OverprintMode {
    /// Erase underlying color when overprinting
    EraseUnderlying, /* 0, default */
    /// Keep underlying color when overprinting
    KeepUnderlying, /* 1 */
}

impl OverprintMode {
    pub fn get_id(&self) -> i64 {
        match self {
            OverprintMode::EraseUnderlying => 0,
            OverprintMode::KeepUnderlying => 1,
        }
    }
}

/// Black generation calculates the amount of black to be used when trying to
/// reproduce a particular color.
#[derive(Debug, PartialEq, Copy, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BlackGenerationFunction {
    /// Regular black generation function
    ///
    /// ```rust,ignore
    /// let cyan = 1.0 - red;
    /// let magenta = 1.0 - green;
    /// let yellow = 1.0 - blue;
    /// let black = min(cyan, magenta, yellow);
    /// ```
    Default,
    /// Expects an UnderColorRemoval to be set. This will compensate
    /// the color for the added black
    ///
    /// ```rust,ignore
    /// let cyan = 1.0 - red;
    /// let magenta = 1.0 - green;
    /// let yellow = 1.0 - blue;
    /// let black = min(cyan, magenta, yellow);
    /// ```
    WithUnderColorRemoval,
}

#[derive(Debug, PartialEq, Copy, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BlackGenerationExtraFunction {}

/// See `BlackGenerationFunction`, too. Undercolor removal reduces the amounts
/// of the cyan, magenta, and yellow components to compensate for the amount of
/// black that was added by black generation.
///
/// The undercolor-removal function computes the amount to subtract from each of
/// the intermediate c, m, and y values to produce the final cyan, magenta, and yellow
/// components. It can simply return its k operand unchanged, or it can return 0.0
/// (so that no color is removed), some fraction of the black amount, or even a
/// negative amount, thereby adding to the total amount of colorant.
#[derive(Debug, PartialEq, Copy, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum UnderColorRemovalFunction {
    Default,
}

#[derive(Debug, PartialEq, Copy, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum UnderColorRemovalExtraFunction {}

#[derive(Debug, PartialEq, Copy, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TransferFunction {}

#[derive(Debug, PartialEq, Copy, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TransferExtraFunction {}

/// In PDF 1.2, the graphics state includes a current halftone parameter,
/// which determines the halftoning process to be used by the painting operators.
/// It may be defined by either a dictionary or a stream, depending on the
/// type of halftone; the term halftone dictionary is used generically
/// throughout this section to refer to either a dictionary object or the
/// dictionary portion of a stream object. (The halftones that are defined
/// by streams are specifically identified as such in the descriptions
/// of particular halftone types; unless otherwise stated, they are
/// understood to be defined by simple dictionaries instead.)
/*
    <<
        /Type /Halftone
        /HalftoneType 1
        /Frequency 120
        /Angle 30
        /SpotFunction /CosineDot
        /TransferFunction /Identity
    >>
*/
/// Deserialized into Integer: 1, 5, 6, 10 or 16
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "type", content = "data")]
pub enum HalftoneType {
    /// 1: Defines a single halftone screen by a frequency, angle, and spot function
    Type1(f32, f32, SpotFunction),
    /// 5: Defines an arbitrary number of halftone screens, one for each colorant or
    /// color component (including both primary and spot colorants).
    /// The keys in this dictionary are names of colorants; the values are halftone
    /// dictionaries of other types, each defining the halftone screen for a single colorant.
    Type5(Vec<HalftoneType>),
    /// 6: Defines a single halftone screen by a threshold array containing 8-bit sample values.
    Type6(Vec<u8>),
    /// 10: Defines a single halftone screen by a threshold array containing 8-bit sample values,
    /// representing a halftone cell that may have a nonzero screen angle.
    Type10(Vec<u8>),
    /// 16: __(PDF 1.3)__ Defines a single halftone screen by a threshold array containing 16-bit
    /// sample values, representing a halftone cell that may have a nonzero screen angle.
    Type16(Vec<u16>),
}

impl HalftoneType {
    /// Get the identifer integer of the HalftoneType
    pub fn get_type(&self) -> i64 {
        use self::HalftoneType::*;
        match *self {
            Type1(_, _, _) => 1,
            Type5(_) => 5, /* this type does not actually exist, todo */
            Type6(_) => 6,
            Type10(_) => 10,
            Type16(_) => 16,
        }
    }
}

/// Spot functions, Table 6.1, Page 489 in Pdf Reference v1.7
/// The code is pseudo code, returning the grey component at (x, y).
#[derive(Debug, PartialEq, Copy, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SpotFunction {
    /// `1 - (pow(x, 2) + pow(y, 2))`
    SimpleDot,
    /// `pow(x, 2) + pow(y, 2) - 1`
    InvertedSimpleDot,
    /// `(sin(360 * x) / 2) + (sin(360 * y) / 2)`
    DoubleDot,
    /// `- ((sin(360 * x) / 2) + (sin(360 * y) / 2))`
    InvertedDoubleDot,
    /// `(cos(180 * x) / 2) + (cos(180 * y) / 2)`
    CosineDot,
    /// `(sin(360 x (x / 2)) / 2) + (sin(360 * y) / 2)`
    Double,
    /// `- ((sin(360 x (x / 2)) / 2) + (sin(360 * y) / 2))`
    InvertedDouble,
    /// `- abs(y)`
    Line,
    /// `x`
    LineX,
    /// `y`
    LineY,
    /// ```rust,ignore
    /// if abs(x) + abs(y) <= 1 {
    ///     1 - (pow(x, 2) + pow(y, 2))
    /// } else {
    ///     pow((abs(x) - 1), 2) + pow((abs(y) - 1), 2) - 1
    /// }
    /// ```
    Round,
    /// ```rust,ignore
    /// let w = (3 * abs(x)) + (4 * abs(y)) - 3;
    ///
    /// if w < 0 {
    ///     1 - ((pow(x, 2) + pow((abs(y) / 0.75), 2)) / 4)
    /// } else if w > 1 {
    ///     pow((pow((1 - abs(x), 2) + (1 - abs(y)) / 0.75), 2) / 4) - 1
    /// } else {
    ///     0.5 - w
    /// }
    /// ```
    Ellipse,
    /// `1 - (pow(x, 2) + 0.9 * pow(y, 2))`
    EllipseA,
    /// `pow(x, 2) + 0.9 * pow(y, 2) - 1`
    InvertedEllipseA,
    /// `1 - sqrt(pow(x, 2) + (5 / 8) * pow(y, 2))`
    EllipseB,
    /// `1 - (0.9 * pow(x, 2) + pow(y, 2))`
    EllipseC,
    /// `0.9 * pow(x, 2) + pow(y, 2) - 1`
    InvertedEllipseC,
    /// `- max(abs(x), abs(y))`
    Square,
    /// `- min(abs(x), abs(y))`
    Cross,
    /// `(0.9 * abs(x) + abs(y)) / 2`
    Rhomboid,
    /// ```rust,ignore
    /// let t = abs(x) + abs(y);
    /// if t <= 0.75 {
    ///     1 - (pow(x, 2) + pow(y, 2))
    /// } else if t < 1.23 {
    ///     1 - (0.85 * abs(x) + abs(y))
    /// } else {
    ///     pow((abs(x) - 1), 2) + pow((abs(y) - 1), 2) - 1
    /// }
    /// ```
    Diamond,
}

#[derive(Debug, PartialEq, Copy, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", untagged)]
pub enum BlendMode {
    Seperable(SeperableBlendMode),
    NonSeperable(NonSeperableBlendMode),
}

impl BlendMode {
    pub fn normal() -> BlendMode {
        BlendMode::Seperable(SeperableBlendMode::Normal)
    }
    pub fn multiply() -> BlendMode {
        BlendMode::Seperable(SeperableBlendMode::Multiply)
    }
    pub fn screen() -> BlendMode {
        BlendMode::Seperable(SeperableBlendMode::Screen)
    }
    pub fn overlay() -> BlendMode {
        BlendMode::Seperable(SeperableBlendMode::Overlay)
    }
    pub fn darken() -> BlendMode {
        BlendMode::Seperable(SeperableBlendMode::Darken)
    }
    pub fn lighten() -> BlendMode {
        BlendMode::Seperable(SeperableBlendMode::Lighten)
    }
    pub fn color_dodge() -> BlendMode {
        BlendMode::Seperable(SeperableBlendMode::ColorDodge)
    }
    pub fn color_burn() -> BlendMode {
        BlendMode::Seperable(SeperableBlendMode::ColorBurn)
    }
    pub fn hard_light() -> BlendMode {
        BlendMode::Seperable(SeperableBlendMode::HardLight)
    }
    pub fn soft_light() -> BlendMode {
        BlendMode::Seperable(SeperableBlendMode::SoftLight)
    }
    pub fn difference() -> BlendMode {
        BlendMode::Seperable(SeperableBlendMode::Difference)
    }
    pub fn exclusion() -> BlendMode {
        BlendMode::Seperable(SeperableBlendMode::Exclusion)
    }
    pub fn hue() -> BlendMode {
        BlendMode::NonSeperable(NonSeperableBlendMode::Hue)
    }
    pub fn saturation() -> BlendMode {
        BlendMode::NonSeperable(NonSeperableBlendMode::Saturation)
    }
    pub fn color() -> BlendMode {
        BlendMode::NonSeperable(NonSeperableBlendMode::Color)
    }
    pub fn luminosity() -> BlendMode {
        BlendMode::NonSeperable(NonSeperableBlendMode::Luminosity)
    }
    pub fn get_id(&self) -> &'static str {
        use self::{BlendMode::*, NonSeperableBlendMode::*, SeperableBlendMode::*};

        match self {
            Seperable(s) => match s {
                Normal => "Normal",
                Multiply => "Multiply",
                Screen => "Screen",
                Overlay => "Overlay",
                Darken => "Darken",
                Lighten => "Lighten",
                ColorDodge => "ColorDodge",
                ColorBurn => "ColorBurn",
                HardLight => "HardLight",
                SoftLight => "SoftLight",
                Difference => "Difference",
                Exclusion => "Exclusion",
            },
            NonSeperable(n) => match n {
                Hue => "Hue",
                Saturation => "Saturation",
                Color => "Color",
                Luminosity => "Luminosity",
            },
        }
    }
}

/// PDF Reference 1.7, Page 520, Table 7.2
/// Blending modes for objects
/// In the following reference, each function gets one new color (the thing to paint on top)
/// and an old color (the color that was already present before the object gets painted)
///
/// The function simply notes the formula that has to be applied to (`color_new`, `color_old`) in
/// order to get the desired effect. You have to run each formula once for each color channel.
#[derive(Debug, PartialEq, Copy, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SeperableBlendMode {
    /// Selects the source color, ignoring the old color. Default mode.
    ///
    /// `color_new`
    Normal,
    /// Multiplies the old color and source color values
    /// Note that these values have to be in the range [0.0 to 1.0] to work.
    /// The result color is always at least as dark as either of the two constituent
    /// colors. Multiplying any color with black produces black; multiplying with white
    /// leaves the original color unchanged.Painting successive overlapping objects with
    /// a color other than black or white produces progressively darker colors.
    ///
    /// `color_old * color_new`
    Multiply,
    /// Multiplies the complements of the old color and new color values, then
    /// complements the result
    /// The result color is always at least as light as either of the two constituent colors.
    /// Screening any color with white produces white; screening with black leaves the original
    /// color unchanged. The effect is similar to projecting multiple photographic slides
    /// simultaneously onto a single screen.
    ///
    /// `color_old + color_new - (color_old * color_new)`
    Screen,
    /// Multiplies or screens the colors, depending on the old color value. Source colors
    /// overlay the old color while preserving its highlights and shadows. The old color is
    /// not replaced but is mixed with the source color to reflect the lightness or darkness
    /// of the old color.
    ///
    /// TLDR: It's the inverse of HardLight
    ///
    /// ```rust,ignore
    /// if color_old <= 0.5 {
    ///     Multiply(color_new, 2 x color_old)
    /// } else {
    ///     Screen(color_new, 2 * color_old - 1)
    /// }
    /// ```
    Overlay,
    /// Selects the darker one of two colors.The old color is replaced with the
    /// new color where the new color is darker; otherwise, it is left unchanged.
    ///
    /// `min(color_old, color_new)`
    Darken,
    /// Selects the lighter one of two colors. The old color is replaced with the
    /// new color where the new color is lighter; otherwise, it is left unchanged.
    ///
    /// `max(color_old, color_new)`
    Lighten,
    /// Brightens the backdrop color to reflect the source color. Painting with
    /// black produces no changes.
    ///
    /// ```rust,ignore
    /// if color_new < 1 {
    ///     min(1, color_old / (1 - color_new))
    /// } else {
    ///     1
    /// }
    /// ```
    ColorDodge,
    /// Darkens the backdrop color to reflect the source color. Painting with
    /// white produces no change.
    ///
    /// ```rust,ignore
    /// if color_new > 0 {
    ///     1 - min(1, (1 - color_old) / color_new)
    /// } else {
    ///     0
    /// }
    /// ```
    ColorBurn,
    /// Multiplies or screens the colors, depending on the source color value. The effect is
    /// similar to shining a harsh spotlight on the old color. It's the inverse of Screen.
    ///
    /// ```rust,ignore
    /// if color_new <= 0.5 {
    ///     Multiply(color_old, 2 x color_new)
    /// } else {
    ///     Screen(color_old, 2 * color_new - 1)
    /// }
    /// ```
    HardLight,
    /// Darkens or lightens the colors, depending on the source color value.
    /// The effect is similar to shining a diffused spotlight on the backdrop.
    ///
    /// ```rust,ignore
    /// if color_new <= 0.5 {
    ///     color_old - ((1 - (2 * color_new)) * color_old * (1 - color_old))
    /// } else {
    ///     let mut dx_factor = color_old.sqrt();
    ///     if color_old <= 0.25 {
    ///         dx_factor = (((16 * color_old - 12) * color_old) + 4) * color_old;
    ///     }
    ///     color_old + ((2 * color_new) - 1) * (dx_factor - color_old)
    /// }
    /// ```
    SoftLight,
    /// Subtracts the darker of the two constituent colors from the lighter color
    /// Painting with white inverts the backdrop color; painting with black produces no change.
    ///
    /// `abs(color_old - color_new)`
    Difference,
    /// Produces an effect similar to that of the Difference mode but lower in contrast.
    /// Painting with white inverts the backdrop color; painting with black produces no change.
    ///
    /// `color_old + color_new - (2 * color_old * color_new)`
    Exclusion,
}

/// Since the nonseparable blend modes consider all color components in combination, their
/// computation depends on the blending color space in which the components are interpreted.
/// They may be applied to all multiple-component color spaces that are allowed as blending
/// color spaces (see Section 7.2.3, “Blending Color Space”).
///
/// All of these blend modes conceptually entail the following steps:
///
/// 1. Convert the backdrop and source colors from the blending color space to an intermediate HSL
///    (hue-saturation-luminosity) representation.
/// 2. Create a new color from some combination of hue, saturation, and luminosity components
///    selected from the backdrop and source colors.
/// 3. Convert the result back to the original (blending) color space.
///
/// However, the formulas given below do not actually perform these conversions. Instead,
/// they start with whichever color (backdrop or source) is providing the hue for the result;
/// then they adjust this color to have the proper saturation and luminosity.
///
/// ### For RGB color spaces
///
/// The nonseparable blend mode formulas make use of several auxiliary functions. These
/// functions operate on colors that are assumed to have red, green, and blue components.
///
/// ```rust,ignore
/// # #[macro_use] extern crate printpdf;
/// # use printpdf::Rgb;
/// # use printpdf::glob_macros::*;
/// # fn main() { /* needed for testing*/ }
/// fn luminosity(input: Rgb) -> f32 {
///     0.3 * input.r + 0.59 * input.g + 0.11 * input.b
/// }
///
/// fn set_luminosity(input: Rgb, target_luminosity: f32) -> Rgb {
///     let d = target_luminosity - luminosity(input);
///     Rgb {
///         r: input.r + d,
///         g: input.g + d,
///         b: input.b + d,
///         icc_profile: input.icc_profile,
///     }
/// }
///
/// fn clip_color(mut input: Rgb) -> Rgb {
///
///     let lum = luminosity(input);
///
///     let mut cur_r = (input.r * 1000.0) as i64;
///     let mut cur_g = (input.g * 1000.0) as i64;
///     let mut cur_b = (input.b * 1000.0) as i64;
///
///     /// min! and max! is defined in printpdf/src/glob_macros.rs
///     let mut min = min!(cur_r, cur_g, cur_b);
///     let mut max = max!(cur_r, cur_g, cur_b);
///
///     let new_min = (min as f32) / 1000.0;
///     let new_max = (max as f32) / 1000.0;
///
///     if new_min < 0.0 {
///         input.r = lum + (((input.r - lum) * lum) / (lum - new_min));
///         input.g = lum + (((input.g - lum) * lum) / (lum - new_min));
///         input.b = lum + (((input.b - lum) * lum) / (lum - new_min));
///     } else if new_max > 1.0 {
///         input.r = lum + ((input.r - lum) * (1.0 - lum) / (new_max - lum));
///         input.g = lum + ((input.g - lum) * (1.0 - lum) / (new_max - lum));
///         input.b = lum + ((input.b - lum) * (1.0 - lum) / (new_max - lum));
///     }
///
///     return input;
/// }
///
/// fn saturation(input: Rgb) -> f32 {
///     let mut cur_r = (input.r * 1000.0) as i64;
///     let mut cur_g = (input.g * 1000.0) as i64;
///     let mut cur_b = (input.b * 1000.0) as i64;
///
///     /// min! and max! is defined in printpdf/src/glob_macros.rs
///     let mut min = min!(cur_r, cur_g, cur_b);
///     let mut max = max!(cur_r, cur_g, cur_b);
///
///     let new_min = (min as f32) / 1000.0;
///     let new_max = (max as f32) / 1000.0;
///     new_max - new_min
/// }
/// ```
///
/// ### For CMYK color spaces
///
/// The C, M, and Y components are converted to their complementary R, G, and B components
/// in the usual way. The formulas above are applied to the RGB color values. The results
/// are converted back to C, M, and Y.
///
/// For the K component, the result is the K component of Cb for the Hue, Saturation, and
/// Color blend modes; it is the K component of Cs for the Luminosity blend mode.
#[derive(Debug, PartialEq, Copy, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NonSeperableBlendMode {
    Hue,
    Saturation,
    Color,
    Luminosity,
}

/* RI name (or ri inside a stream) */
/// Although CIE-based color specifications are theoretically device-independent,
/// they are subject to practical limitations in the color reproduction capabilities of
/// the output device. Such limitations may sometimes require compromises to be
/// made among various properties of a color specification when rendering colors for
/// a given device. Specifying a rendering intent (PDF 1.1) allows a PDF file to set priorities
/// regarding which of these properties to preserve and which to sacrifice.
#[derive(Debug, PartialEq, Copy, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RenderingIntent {
    /// Colors are represented solely with respect to the light source; no
    /// correction is made for the output medium’s white point (such as
    /// the color of unprinted paper). Thus, for example, a monitor’s
    /// white point, which is bluish compared to that of a printer’s paper,
    /// would be reproduced with a blue cast. In-gamut colors are
    /// reproduced exactly; out-of-gamut colors are mapped to the
    /// nearest value within the reproducible gamut. This style of reproduction
    /// has the advantage of providing exact color matches
    /// from one output medium to another. It has the disadvantage of
    /// causing colors with Y values between the medium’s white point
    /// and 1.0 to be out of gamut. A typical use might be for logos and
    /// solid colors that require exact reproduction across different media.
    AbsoluteColorimetric,
    /// Colors are represented with respect to the combination of the
    /// light source and the output medium’s white point (such as the
    /// color of unprinted paper). Thus, for example, a monitor’s white
    /// point would be reproduced on a printer by simply leaving the
    /// paper unmarked, ignoring color differences between the two
    /// media. In-gamut colors are reproduced exactly; out-of-gamut
    /// colors are mapped to the nearest value within the reproducible
    /// gamut. This style of reproduction has the advantage of adapting
    /// for the varying white points of different output media. It has the
    /// disadvantage of not providing exact color matches from one me-
    /// dium to another. A typical use might be for vector graphics.
    RelativeColorimetric,
    /// Colors are represented in a manner that preserves or emphasizes
    /// saturation. Reproduction of in-gamut colors may or may not be
    /// colorimetrically accurate. A typical use might be for business
    /// graphics, where saturation is the most important attribute of the
    /// color.
    Saturation,
    /// Colors are represented in a manner that provides a pleasing perceptual
    /// appearance. To preserve color relationships, both in-gamut
    /// and out-of-gamut colors are generally modified from
    /// their precise colorimetric values. A typical use might be for scanned images.
    Perceptual,
}

impl RenderingIntent {
    pub fn get_id(&self) -> &'static str {
        use self::RenderingIntent::*;
        match self {
            AbsoluteColorimetric => "AbsoluteColorimetric",
            RelativeColorimetric => "RelativeColorimetric",
            Saturation => "Saturation",
            Perceptual => "Perceptual",
        }
    }
}

/// A soft mask is used for transparent images such as PNG with an alpha component
/// The bytes range from 0xFF (opaque) to 0x00 (transparent). The alpha channel of a
/// PNG image have to be sorted out.
/// Can also be used for Vignettes, etc.
/// Beware of color spaces!
/// __See PDF Reference Page 545__ - Soft masks
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SoftMask {
    /// The data to be used as a soft mask
    data: Vec<u8>,
    /// Bits per component (1 for black / white, 8 for greyscale, up to 16)
    bits_per_component: u8,
}

#[derive(Debug, PartialEq, Copy, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SoftMaskFunction {
    // (Color, Shape, Alpha) = Composite(Color0, Alpha0, Group)
    /// In this function, the old (backdrop) color does not contribute to the result.
    /// This is the easies function, but may look bad at edges.
    GroupAlpha,
    //
    GroupLuminosity,
}
