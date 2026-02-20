use core::fmt;
use std::collections::HashSet;

use lopdf::Dictionary as LoDictionary;
use serde_derive::{Deserialize, Serialize};

use crate::{
    units::{Mm, Pt},
    BuiltinFont, FontId,
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
#[derive(PartialEq, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rect {
    pub x: Pt,
    pub y: Pt,
    pub width: Pt,
    pub height: Pt,
    pub mode: Option<PaintMode>,
    pub winding_order: Option<WindingOrder>,
}

impl fmt::Debug for Rect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}x{} @ {} - {}",
            self.width.0, self.height.0, self.x.0, self.y.0
        )
    }
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
            mode: None,
            winding_order: None,
        }
    }

    pub fn from_xywh(x: Pt, y: Pt, width: Pt, height: Pt) -> Self {
        Self {
            x,
            y,
            width,
            height,
            mode: None,
            winding_order: None,
        }
    }

    pub fn to_polygon(&self) -> Polygon {
        Polygon {
            rings: vec![PolygonRing {
                points: self.gen_points(),
            }],
            mode: self.mode.unwrap_or(PaintMode::Fill),
            winding_order: self.winding_order.unwrap_or(WindingOrder::NonZero),
        }
    }

    pub fn to_line(&self) -> Line {
        Line {
            points: self.gen_points(),
            is_closed: true,
        }
    }

    fn gen_points(&self) -> Vec<LinePoint> {
        let top = Pt(self.y.0 + self.height.0);
        let bottom = self.y;
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
#[derive(Debug, Clone, Default, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LineDashPattern {
    /// Offset at which the dashing pattern should start, measured from the beginning ot the line
    /// Default: 0 (start directly where the line starts)
    pub offset: f32,
    /// Dash, gap, dash, gap, ... 
    pub pattern: heapless::Vec<f32, 6, u32>,
}

impl LineDashPattern {
    pub fn get_svg_id(&self) -> String {
        self.pattern
            .iter()
            .map(|num| num.to_string())
            .collect::<Vec<_>>()
            .join(",")
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
#[serde(rename = "kebab-case")]
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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
#[serde(rename_all = "kebab-case")]
pub enum BuiltinOrExternalFontId {
    // One of the 12 default PDF fonts
    Builtin(BuiltinFont),
    /// External, third-party font
    External(FontId),
}

impl BuiltinOrExternalFontId {
    pub fn is_builtin(&self) -> bool {
        match self {
            BuiltinOrExternalFontId::Builtin(_) => true,
            BuiltinOrExternalFontId::External(_) => false,
        }
    }

    pub fn get_id(&self) -> &str {
        match self {
            BuiltinOrExternalFontId::Builtin(builtin_font) => builtin_font.get_id(),
            BuiltinOrExternalFontId::External(font_id) => &font_id.0,
        }
    }
    pub fn from_str(s: &str) -> Self {
        if let Some(bf) = BuiltinFont::from_id(s) {
            Self::Builtin(bf)
        } else {
            Self::External(FontId(s.to_string()))
        }
    }
}

/// PDF ExtGState dictionary
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtendedGraphicsState {
    /// Tracks changed fields using strongly typed enum
    pub(crate) changed_fields: HashSet<ChangedField>,
    /// LW - Line width (PDF 1.3)
    pub(crate) line_width: f32,
    /// LC - Line cap style (PDF 1.3)
    pub(crate) line_cap: LineCapStyle,
    /// LJ - Line join style (PDF 1.3)
    pub(crate) line_join: LineJoinStyle,
    /// ML - Miter limit (PDF 1.3)
    pub(crate) miter_limit: f32,
    /// D - Line dash pattern (PDF 1.3)
    pub(crate) line_dash_pattern: Option<LineDashPattern>,
    /// RI - Rendering intent name (PDF 1.3)
    pub(crate) rendering_intent: RenderingIntent,
    /// OP - Overprint for stroking
    pub(crate) overprint_stroke: bool,
    /// op - Overprint for nonstroking (PDF 1.3)
    pub(crate) overprint_fill: bool,
    /// OPM - Overprint mode (PDF 1.3)
    pub(crate) overprint_mode: OverprintMode,
    /// Font array
    pub(crate) font: Option<BuiltinOrExternalFontId>,
    /// BG - Black-generation function
    pub(crate) black_generation: Option<BlackGenerationFunction>,
    /// BG2 - Black-generation extra function (PDF 1.3)
    pub(crate) black_generation_extra: Option<BlackGenerationExtraFunction>,
    /// UCR - Undercolor-removal function
    pub(crate) under_color_removal: Option<UnderColorRemovalFunction>,
    /// UCR2 - Undercolor-removal extra function (PDF 1.3)
    pub(crate) under_color_removal_extra: Option<UnderColorRemovalExtraFunction>,
    /// TR - Transfer function
    pub(crate) transfer_function: Option<TransferFunction>,
    /// TR2 - Transfer extra function (PDF 1.3)
    pub(crate) transfer_extra_function: Option<TransferExtraFunction>,
    /// HT - Halftone dictionary/stream/name
    pub(crate) halftone_dictionary: Option<HalftoneType>,
    /// FL - Flatness tolerance (PDF 1.3)
    pub(crate) flatness_tolerance: f32,
    /// SM - Smoothness tolerance (PDF 1.3)
    pub(crate) smoothness_tolerance: f32,
    /// SA - Automatic stroke adjustment
    pub(crate) stroke_adjustment: bool,
    /// BM - Blend mode (PDF 1.4)
    pub(crate) blend_mode: BlendMode,
    /// SM - Soft mask (PDF 1.4)
    pub(crate) soft_mask: Option<SoftMask>,
    /// CA - Stroking alpha constant (PDF 1.4)
    pub(crate) current_stroke_alpha: f32,
    /// ca - Nonstroking alpha constant (PDF 1.4)
    pub(crate) current_fill_alpha: f32,
    /// AIS - Alpha source flag (PDF 1.4)
    pub(crate) alpha_is_shape: bool,
    /// TK - Text knockout flag (PDF 1.4)
    pub(crate) text_knockout: bool,
}

// Implement getter methods for all fields
impl ExtendedGraphicsState {
    // Getter methods for all fields
    pub fn line_width(&self) -> f32 {
        self.line_width
    }

    pub fn line_cap(&self) -> LineCapStyle {
        self.line_cap
    }

    pub fn line_join(&self) -> LineJoinStyle {
        self.line_join
    }

    pub fn miter_limit(&self) -> f32 {
        self.miter_limit
    }

    pub fn line_dash_pattern(&self) -> &Option<LineDashPattern> {
        &self.line_dash_pattern
    }

    pub fn rendering_intent(&self) -> RenderingIntent {
        self.rendering_intent
    }

    pub fn overprint_stroke(&self) -> bool {
        self.overprint_stroke
    }

    pub fn overprint_fill(&self) -> bool {
        self.overprint_fill
    }

    pub fn overprint_mode(&self) -> OverprintMode {
        self.overprint_mode
    }

    pub fn font(&self) -> &Option<BuiltinOrExternalFontId> {
        &self.font
    }

    pub fn black_generation(&self) -> &Option<BlackGenerationFunction> {
        &self.black_generation
    }

    pub fn black_generation_extra(&self) -> &Option<BlackGenerationExtraFunction> {
        &self.black_generation_extra
    }

    pub fn under_color_removal(&self) -> &Option<UnderColorRemovalFunction> {
        &self.under_color_removal
    }

    pub fn under_color_removal_extra(&self) -> &Option<UnderColorRemovalExtraFunction> {
        &self.under_color_removal_extra
    }

    pub fn transfer_function(&self) -> &Option<TransferFunction> {
        &self.transfer_function
    }

    pub fn transfer_extra_function(&self) -> &Option<TransferExtraFunction> {
        &self.transfer_extra_function
    }

    pub fn halftone_dictionary(&self) -> &Option<HalftoneType> {
        &self.halftone_dictionary
    }

    pub fn flatness_tolerance(&self) -> f32 {
        self.flatness_tolerance
    }

    pub fn smoothness_tolerance(&self) -> f32 {
        self.smoothness_tolerance
    }

    pub fn stroke_adjustment(&self) -> bool {
        self.stroke_adjustment
    }

    pub fn blend_mode(&self) -> &BlendMode {
        &self.blend_mode
    }

    pub fn soft_mask(&self) -> &Option<SoftMask> {
        &self.soft_mask
    }

    pub fn current_stroke_alpha(&self) -> f32 {
        self.current_stroke_alpha
    }

    pub fn current_fill_alpha(&self) -> f32 {
        self.current_fill_alpha
    }

    pub fn alpha_is_shape(&self) -> bool {
        self.alpha_is_shape
    }

    pub fn text_knockout(&self) -> bool {
        self.text_knockout
    }

    // Setter methods (mainly for internal use and deserialization)
    // Each setter should also update the changed_fields set
    pub fn set_line_width(&mut self, value: f32) {
        self.line_width = value;
        self.changed_fields.insert(ChangedField::LineWidth);
    }

    pub fn set_line_cap(&mut self, value: LineCapStyle) {
        self.line_cap = value;
        self.changed_fields.insert(ChangedField::LineCap);
    }

    pub fn set_line_join(&mut self, value: LineJoinStyle) {
        self.line_join = value;
        self.changed_fields.insert(ChangedField::LineJoin);
    }

    pub fn set_miter_limit(&mut self, value: f32) {
        self.miter_limit = value;
        self.changed_fields.insert(ChangedField::MiterLimit);
    }

    pub fn set_line_dash_pattern(&mut self, value: Option<LineDashPattern>) {
        self.line_dash_pattern = value;
        self.changed_fields.insert(ChangedField::LineDashPattern);
    }

    pub fn set_rendering_intent(&mut self, value: RenderingIntent) {
        self.rendering_intent = value;
        self.changed_fields.insert(ChangedField::RenderingIntent);
    }

    pub fn set_overprint_stroke(&mut self, value: bool) {
        self.overprint_stroke = value;
        self.changed_fields.insert(ChangedField::OverprintStroke);
    }

    pub fn set_overprint_fill(&mut self, value: bool) {
        self.overprint_fill = value;
        self.changed_fields.insert(ChangedField::OverprintFill);
    }

    pub fn set_overprint_mode(&mut self, value: OverprintMode) {
        self.overprint_mode = value;
        self.changed_fields.insert(ChangedField::OverprintMode);
    }

    pub fn set_font(&mut self, value: Option<BuiltinOrExternalFontId>) {
        self.font = value;
        self.changed_fields.insert(ChangedField::Font);
    }

    pub fn set_black_generation(&mut self, value: Option<BlackGenerationFunction>) {
        self.black_generation = value;
        self.changed_fields.insert(ChangedField::BlackGeneration);
    }

    pub fn set_black_generation_extra(&mut self, value: Option<BlackGenerationExtraFunction>) {
        self.black_generation_extra = value;
        self.changed_fields
            .insert(ChangedField::BlackGenerationExtra);
    }

    pub fn set_under_color_removal(&mut self, value: Option<UnderColorRemovalFunction>) {
        self.under_color_removal = value;
        self.changed_fields.insert(ChangedField::UnderColorRemoval);
    }

    pub fn set_under_color_removal_extra(&mut self, value: Option<UnderColorRemovalExtraFunction>) {
        self.under_color_removal_extra = value;
        self.changed_fields
            .insert(ChangedField::UnderColorRemovalExtra);
    }

    pub fn set_transfer_function(&mut self, value: Option<TransferFunction>) {
        self.transfer_function = value;
        self.changed_fields.insert(ChangedField::TransferFunction);
    }

    pub fn set_transfer_extra_function(&mut self, value: Option<TransferExtraFunction>) {
        self.transfer_extra_function = value;
        self.changed_fields
            .insert(ChangedField::TransferFunctionExtra);
    }

    pub fn set_halftone_dictionary(&mut self, value: Option<HalftoneType>) {
        self.halftone_dictionary = value;
        self.changed_fields.insert(ChangedField::HalftoneDictionary);
    }

    pub fn set_flatness_tolerance(&mut self, value: f32) {
        self.flatness_tolerance = value;
        self.changed_fields.insert(ChangedField::FlatnessTolerance);
    }

    pub fn set_smoothness_tolerance(&mut self, value: f32) {
        self.smoothness_tolerance = value;
        self.changed_fields
            .insert(ChangedField::SmoothnessTolerance);
    }

    pub fn set_stroke_adjustment(&mut self, value: bool) {
        self.stroke_adjustment = value;
        self.changed_fields.insert(ChangedField::StrokeAdjustment);
    }

    pub fn set_blend_mode(&mut self, value: BlendMode) {
        self.blend_mode = value;
        self.changed_fields.insert(ChangedField::BlendMode);
    }

    pub fn set_soft_mask(&mut self, value: Option<SoftMask>) {
        self.soft_mask = value;
        self.changed_fields.insert(ChangedField::SoftMask);
    }

    pub fn set_current_stroke_alpha(&mut self, value: f32) {
        self.current_stroke_alpha = value;
        self.changed_fields.insert(ChangedField::CurrentStrokeAlpha);
    }

    pub fn set_current_fill_alpha(&mut self, value: f32) {
        self.current_fill_alpha = value;
        self.changed_fields.insert(ChangedField::CurrentFillAlpha);
    }

    pub fn set_alpha_is_shape(&mut self, value: bool) {
        self.alpha_is_shape = value;
        self.changed_fields.insert(ChangedField::AlphaIsShape);
    }

    pub fn set_text_knockout(&mut self, value: bool) {
        self.text_knockout = value;
        self.changed_fields.insert(ChangedField::TextKnockout);
    }

    // A method to check if a field has been changed
    pub fn has_changed(&self, field: ChangedField) -> bool {
        self.changed_fields.contains(&field)
    }

    /// Set line width and return self
    pub fn with_line_width(mut self, width: f32) -> Self {
        self.set_line_width(width);
        self
    }

    /// Set line cap style and return self
    pub fn with_line_cap(mut self, cap: LineCapStyle) -> Self {
        self.set_line_cap(cap);
        self
    }

    /// Set line join style and return self
    pub fn with_line_join(mut self, join: LineJoinStyle) -> Self {
        self.set_line_join(join);
        self
    }

    /// Set miter limit and return self
    pub fn with_miter_limit(mut self, limit: f32) -> Self {
        self.set_miter_limit(limit);
        self
    }

    /// Set line dash pattern and return self
    pub fn with_line_dash_pattern(mut self, pattern: Option<LineDashPattern>) -> Self {
        self.set_line_dash_pattern(pattern);
        self
    }

    /// Set rendering intent and return self
    pub fn with_rendering_intent(mut self, intent: RenderingIntent) -> Self {
        self.set_rendering_intent(intent);
        self
    }

    /// Set overprint stroke and return self
    pub fn with_overprint_stroke(mut self, overprint: bool) -> Self {
        self.set_overprint_stroke(overprint);
        self
    }

    /// Set overprint fill and return self
    pub fn with_overprint_fill(mut self, overprint: bool) -> Self {
        self.set_overprint_fill(overprint);
        self
    }

    /// Set overprint mode and return self
    pub fn with_overprint_mode(mut self, mode: OverprintMode) -> Self {
        self.set_overprint_mode(mode);
        self
    }

    /// Set font and return self
    pub fn with_font(mut self, font: Option<BuiltinOrExternalFontId>) -> Self {
        self.set_font(font);
        self
    }

    /// Set black generation function and return self
    pub fn with_black_generation(mut self, func: Option<BlackGenerationFunction>) -> Self {
        self.set_black_generation(func);
        self
    }

    /// Set black generation extra function and return self
    pub fn with_black_generation_extra(
        mut self,
        func: Option<BlackGenerationExtraFunction>,
    ) -> Self {
        self.set_black_generation_extra(func);
        self
    }

    /// Set under color removal function and return self
    pub fn with_under_color_removal(mut self, func: Option<UnderColorRemovalFunction>) -> Self {
        self.set_under_color_removal(func);
        self
    }

    /// Set under color removal extra function and return self
    pub fn with_under_color_removal_extra(
        mut self,
        func: Option<UnderColorRemovalExtraFunction>,
    ) -> Self {
        self.set_under_color_removal_extra(func);
        self
    }

    /// Set transfer function and return self
    pub fn with_transfer_function(mut self, func: Option<TransferFunction>) -> Self {
        self.set_transfer_function(func);
        self
    }

    /// Set transfer extra function and return self
    pub fn with_transfer_extra_function(mut self, func: Option<TransferExtraFunction>) -> Self {
        self.set_transfer_extra_function(func);
        self
    }

    /// Set halftone dictionary and return self
    pub fn with_halftone_dictionary(mut self, dict: Option<HalftoneType>) -> Self {
        self.set_halftone_dictionary(dict);
        self
    }

    /// Set flatness tolerance and return self
    pub fn with_flatness_tolerance(mut self, tolerance: f32) -> Self {
        self.set_flatness_tolerance(tolerance);
        self
    }

    /// Set smoothness tolerance and return self
    pub fn with_smoothness_tolerance(mut self, tolerance: f32) -> Self {
        self.set_smoothness_tolerance(tolerance);
        self
    }

    /// Set stroke adjustment and return self
    pub fn with_stroke_adjustment(mut self, adjustment: bool) -> Self {
        self.set_stroke_adjustment(adjustment);
        self
    }

    /// Set blend mode and return self
    pub fn with_blend_mode(mut self, mode: BlendMode) -> Self {
        self.set_blend_mode(mode);
        self
    }

    /// Set soft mask and return self
    pub fn with_soft_mask(mut self, mask: Option<SoftMask>) -> Self {
        self.set_soft_mask(mask);
        self
    }

    /// Set current stroke alpha and return self
    pub fn with_current_stroke_alpha(mut self, alpha: f32) -> Self {
        self.set_current_stroke_alpha(alpha);
        self
    }

    /// Set current fill alpha and return self
    pub fn with_current_fill_alpha(mut self, alpha: f32) -> Self {
        self.set_current_fill_alpha(alpha);
        self
    }

    /// Set alpha is shape and return self
    pub fn with_alpha_is_shape(mut self, is_shape: bool) -> Self {
        self.set_alpha_is_shape(is_shape);
        self
    }

    /// Set text knockout and return self
    pub fn with_text_knockout(mut self, knockout: bool) -> Self {
        self.set_text_knockout(knockout);
        self
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
    if let Some(ldp) = &val.line_dash_pattern {
        if val.changed_fields.contains(&ChangedField::LineDashPattern) {
            let array = ldp.pattern.iter().copied().map(Real).collect();
            gs_operations.push(("D".to_string(), Array(array)));
        }
    }

    if let Some(font) = val.font.as_ref() {
        if val.changed_fields.contains(&ChangedField::Font) {
            gs_operations.push(("Font".to_string(), Name(font.get_id().as_bytes().to_vec())));
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
