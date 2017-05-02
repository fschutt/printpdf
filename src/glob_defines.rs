//! PDF constants for tags for reducing typing mistakes

const PDF_TAG_COLOR_SPACE_LINE: &'static str       = "cs";
const PDF_TAG_COLOR_SPACE_FILL: &'static str       = "CS";
const PDF_TAG_SET_COLOR_SPACE_LINE: &'static str   = "SCN";
const PDF_TAG_SET_COLOR_SPACE_FILL: &'static str   = "scn";

const PDF_TAG_BEGIN_TEXT: &'static str             = "BT";
const PDF_TAG_SET_TEXT_FONT: &'static str          = "Tf";
const PDF_TAG_SET_TEXT_POSTION: &'static str       = "Td";
const PDF_TAG_WRITE_TEXT_NO_KERN: &'static str     = "Tj";
const PDF_TAG_WRITE_TEXT_WITH_KERN: &'static str   = "TJ";
const PDF_TAG_END_TEXT: &'static str               = "ET";

const PDF_TAG_SET_LINE_WIDTH: &'static str         = "ET";
const PDF_TAG_MOVE_TO: &'static str                = "w";
const PDF_TAG_3BEZIER_CURVE_V1: &'static str       = "v";
const PDF_TAG_3BEZIER_CURVE_V2: &'static str       = "y";
const PDF_TAG_4BEZIER_CURVE: &'static str          = "c";
const PDF_TAG_END_LINE_OUTLINE: &'static str       = "b";
const PDF_TAG_END_LINE_FILL: &'static str          = "S";