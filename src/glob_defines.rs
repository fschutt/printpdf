//! PDF constants for tags for reducing typing mistakes

pub(crate) const PDF_TAG_COLOR_SPACE_LINE: &'static str       = "cs";
pub(crate) const PDF_TAG_COLOR_SPACE_FILL: &'static str       = "CS";
pub(crate) const PDF_TAG_SET_COLOR_SPACE_LINE: &'static str   = "SCN";
pub(crate) const PDF_TAG_SET_COLOR_SPACE_FILL: &'static str   = "scn";

pub(crate) const PDF_TAG_BEGIN_TEXT: &'static str             = "BT";
pub(crate) const PDF_TAG_SET_TEXT_FONT: &'static str          = "Tf";
pub(crate) const PDF_TAG_SET_TEXT_POSTION: &'static str       = "Td";
pub(crate) const PDF_TAG_WRITE_TEXT_NO_KERN: &'static str     = "Tj";
pub(crate) const PDF_TAG_WRITE_TEXT_WITH_KERN: &'static str   = "TJ";
pub(crate) const PDF_TAG_END_TEXT: &'static str               = "ET";

pub(crate) const PDF_TAG_SET_LINE_WIDTH: &'static str         = "ET";
pub(crate) const PDF_TAG_MOVE_TO: &'static str                = "w";
pub(crate) const PDF_TAG_3BEZIER_CURVE_V1: &'static str       = "v";
pub(crate) const PDF_TAG_3BEZIER_CURVE_V2: &'static str       = "y";
pub(crate) const PDF_TAG_4BEZIER_CURVE: &'static str          = "c";
pub(crate) const PDF_TAG_END_LINE_OUTLINE: &'static str       = "b";
pub(crate) const PDF_TAG_END_LINE_FILL: &'static str          = "S";