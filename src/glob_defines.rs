#![allow(dead_code)]

//! PDF constants for tags for reducing typing mistakes

/*

/// Special graphics state
"q";
"Q";
"cm";

/// Text objects
"BT";
"ET";

/// Text state
"Tc";
"Tw";
"Tz";
"TL";
"Tf";
"Tr";
"Ts";

/// Text positioning
"Td";
"TD";
"Tm";
"Tm";
"T*";

/// Text showing
"Tj";
"TJ";

/// Type 3 fonts
"d0";
"d1";

/// Shading patterns
"sh";

/// Inline images
"BI";
"ID";
"EI";

/// XObjects
"Do";

/// Marked content
"MP";
"DP";
"BMC";
"BDC";
"EMC";

/// Compatability
"BX";
"EX";
*/

/*
pub const PDF_TAG_COLOR_SPACE_LINE: &'static str       = "cs";
pub const PDF_TAG_COLOR_SPACE_FILL: &'static str       = "CS";
pub const PDF_TAG_SET_COLOR_SPACE_LINE: &'static str   = "SCN";
pub const PDF_TAG_SET_COLOR_SPACE_FILL: &'static str   = "scn";

pub const PDF_TAG_BEGIN_TEXT: &'static str             = "BT";
pub const PDF_TAG_SET_TEXT_FONT: &'static str          = "Tf";
pub const PDF_TAG_SET_TEXT_POSTION: &'static str       = "Td";
pub const PDF_TAG_WRITE_TEXT_NO_KERN: &'static str     = "Tj";
pub const PDF_TAG_WRITE_TEXT_WITH_KERN: &'static str   = "TJ";
pub const PDF_TAG_END_TEXT: &'static str               = "ET";

pub const PDF_TAG_SET_LINE_WIDTH: &'static str         = "ET";
pub const PDF_TAG_MOVE_TO: &'static str                = "w";
pub const PDF_TAG_3BEZIER_CURVE_V1: &'static str       = "v";
pub const PDF_TAG_3BEZIER_CURVE_V2: &'static str       = "y";
pub const PDF_TAG_4BEZIER_CURVE: &'static str          = "c";
*/


/// ## General graphics state

/// Set line width
pub const OP_PATH_STATE_SET_LINE_WIDTH: &str                 = "w";
/// Set line join
pub const OP_PATH_STATE_SET_LINE_JOIN: &str                  = "J";
/// Set line cap
pub const OP_PATH_STATE_SET_LINE_CAP: &str                   = "j";
/// Set miter limit
pub const OP_PATH_STATE_SET_MITER_LIMIT: &str                = "M";
/// Set line dash pattern
pub const OP_PATH_STATE_SET_LINE_DASH: &str                  = "d";
/// Set rendering intent
pub const OP_PATH_STATE_SET_RENDERING_INTENT: &str           = "ri";
/// Set flatness tolerance
pub const OP_PATH_STATE_SET_FLATNESS_TOLERANCE: &str         = "i";
/// (PDF 1.2) Set graphics state from parameter dictionary
pub const OP_PATH_STATE_SET_GS_FROM_PARAM_DICT: &str         = "gs";


/// ## Color

/// stroking color space (PDF 1.1)
pub const OP_COLOR_SET_STROKE_CS: &str                       = "CS";
/// non-stroking color space (PDF 1.1)
pub const OP_COLOR_SET_FILL_CS: &str                         = "cs";
/// set stroking color (PDF 1.1)
pub const OP_COLOR_SET_STROKE_COLOR: &str                    = "SC";
/// set stroking color (PDF 1.2) with support for ICC, etc.
pub const OP_COLOR_SET_STROKE_COLOR_ICC: &str                = "SCN";
/// set fill color (PDF 1.1)
pub const OP_COLOR_SET_FILL_COLOR: &str                      = "sc";
/// set fill color (PDF 1.2) with support for Icc, etc.
pub const OP_COLOR_SET_FILL_COLOR_ICC: &str                  = "scn";

/// Set the stroking color space to DeviceGray
pub const OP_COLOR_SET_STROKE_CS_DEVICEGRAY: &str            = "G";
/// Set the fill color space to DeviceGray
pub const OP_COLOR_SET_FILL_CS_DEVICEGRAY: &str              = "g";
/// Set the stroking color space to DeviceRGB
pub const OP_COLOR_SET_STROKE_CS_DEVICERGB: &str             = "RG";
/// Set the fill color space to DeviceRGB
pub const OP_COLOR_SET_FILL_CS_DEVICERGB: &str               = "rg";
/// Set the stroking color space to DeviceCMYK
pub const OP_COLOR_SET_STROKE_CS_DEVICECMYK: &str            = "K";
/// Set the fill color to DeviceCMYK
pub const OP_COLOR_SET_FILL_CS_DEVICECMYK: &str              = "k";


/// Path construction

/// Move to point
pub const OP_PATH_CONST_MOVE_TO: &str                        = "m";
/// Straight line to the two following points
pub const OP_PATH_CONST_LINE_TO: &str                        = "l";
/// Cubic bezier over four following points
pub const OP_PATH_CONST_4BEZIER: &str                        = "c";
/// Cubic bezier with two points in v1
pub const OP_PATH_CONST_3BEZIER_V1: &str                     = "v";
/// Cubic bezier with two points in v2
pub const OP_PATH_CONST_3BEZIER_V2: &str                     = "y";
/// Add rectangle to the path (width / height): x y width height re
pub const OP_PATH_CONST_RECT: &str                           = "re";
/// Close current sub-path (for appending custom patterns along line)
pub const OP_PATH_CONST_CLOSE_SUBPATH: &str                  = "h";
/// Current path is a clip path, non-zero winding order (usually in like `h W S`)
pub const OP_PATH_CONST_CLIP_NZ: &str = "W";
/// Current path is a clip path, non-zero winding order
pub const OP_PATH_CONST_CLIP_EO: &str = "W*";

/// Path painting

/// Stroke path
pub const OP_PATH_PAINT_STROKE: &str                         = "S";
/// Close and stroke path
pub const OP_PATH_PAINT_STROKE_CLOSE: &str                   = "s";
/// Fill path using nonzero winding number rule
pub const OP_PATH_PAINT_FILL_NZ: &str                        = "f";
/// Fill path using nonzero winding number rule (obsolete)
pub const OP_PATH_PAINT_FILL_NZ_OLD: &str                    = "F";
/// Fill path using even-odd rule
pub const OP_PATH_PAINT_FILL_EO: &str                        = "f*";
/// Fill and stroke path using nonzero winding number rule
pub const OP_PATH_PAINT_FILL_STROKE_NZ: &str                 = "B";
/// Close, fill and stroke path using nonzero winding number rule
pub const OP_PATH_PAINT_FILL_STROKE_CLOSE_NZ: &str           = "b";
/// Fill and stroke path using even-odd rule
pub const OP_PATH_PAINT_FILL_STROKE_EO: &str                 = "B*";
/// Close, fill and stroke path using even odd rule
pub const OP_PATH_PAINT_FILL_STROKE_CLOSE_EO: &str           = "b*";
/// End path without filling or stroking
pub const OP_PATH_PAINT_END: &str                            = "n";

/// These color profiles are included in the binary
pub const ICC_PROFILE_ECI_V2: &[u8] = include_bytes!("templates/CoatedFOGRA39.icc");