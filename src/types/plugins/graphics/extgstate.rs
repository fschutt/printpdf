//! Extended graphics state, for advanced graphical operation (overprint, black point control, etc.)
//! 
//! Some of the operations can be done on the layer directly, but for advanced graphics,
//! you need to set the graphics state. A PDF has an internal default graphics state,
//! which can be reset to by setting "ExtendedGraphicsState::default()" as the active gs 
//! dictionary. Setting a new graphics state overwrites the old one, there is no "undo".
//!
//! In order to use a graphics state, it must be added to the Pages resource dicitionary.
//! This is done by the `layer.set_graphics_state()` function, which returns a reference with the name of 
//! the newly added dictionary. From inside a stream, the graphics state parameter is invoked 
//! with the "gs" command using the name of the graphics state as a operator.
//! This is done using the `layer.use_graphics_state()`.
//! 
//! A full graphics state change is done like this:
//!
//! ```rust,ignore
//! let mut new_state = ExtendedGraphicsState::default();
//! new_state.
//! ```

use lopdf;
use lopdf::content::Operation;
use lopdf::Object::*;
use traits::{IntoPdfObject, IntoPdfStreamOperation};
use std::string::String;
use *;

/// ExtGState dictionary
#[derive(Debug, PartialEq, Clone)]
pub struct ExtendedGraphicsState {
	/* /Type ExtGState */

	/* LW float */
	/// __(Optional; PDF 1.3)__ The current line width
	pub line_width: f64,

	/* LC integer */
	/// __(Optional; PDF 1.3)__ The current line cap style
	pub line_cap: LineCapStyle,

	/* LJ integer */
	/// __(Optional; PDF 1.3)__ The current line join style
	pub line_join: LineJoinStyle,

	/* ML float */
	/// __(Optional; PDF 1.3)__ The miter limit (see “Miter Limit” on page 217).
	pub miter_limit: f64,

	/* D array */
	///	__(Optional; PDF 1.3)__ The line dash pattern, expressed as an array of the form
	///	[ dashArray dashPhase ] , where dashArray is itself an array and dashPhase is an
	///	integer (see “Line Dash Pattern” on page 217).
	pub line_dash_pattern: Option<LineDashPattern>,

	/* RI name (or ri inside a stream)*/
	///	__(Optional; PDF 1.3)__ The name of the rendering intent (see “Rendering
	///	Intents” on page 260).
	pub rendering_intent: RenderingIntent,

	/* OP boolean */
	///	__(Optional)__ A flag specifying whether to apply overprint (see Section 4.5.6,
	///	“Overprint Control”). In PDF 1.2 and earlier, there is a single overprint
	///	parameter that applies to all painting operations. Beginning with PDF 1.3,
	///	there are two separate overprint parameters: one for stroking and one for all
	///	other painting operations. Specifying an OP entry sets both parameters un-
	///	less there is also an op entry in the same graphics state parameter dictionary,
	///	in which case the OP entry sets only the overprint parameter for stroking.
	pub overprint_stroke: bool,

	/* op boolean */
	///	__(Optional; PDF 1.3)__ A flag specifying whether to apply overprint (see Section
	///	4.5.6, “Overprint Control”) for painting operations other than stroking. If
	///	this entry is absent, the OP entry, if any, sets this parameter.
	pub overprint_fill: bool,

	/* OPM integer */
	/// __(Optional; PDF 1.3)__ The overprint mode (see Section 4.5.6, “Overprint Control”)
	/// Initial value: `EraseUnderlying`
	pub overprint_mode: OverprintMode,

	/* Font array */
	/// Font structure, expects a dictionary, todo.
	/// *WARNING: DO NOT USE THIS OUTSIDE OF THIS CRATE, YOU MIGHT DAMAGE THE PDF*
	/// Instead, use the facilities provided by the document, such as `doc.add_font()`
	/// and `layer.use_text( [...], font)`. Otherwise, there is no guarantee that the PDF
	/// will show up correct
	pub font: Option<Font>,

	/* BG function */
	///	__(Optional)__ The black-generation function, which maps the interval [ 0.0 1.0 ]
	///	to the interval [ 0.0 1.0 ] (see Section 6.2.3, “Conversion from DeviceRGB to
	///	DeviceCMYK”)
	pub black_generation: Option<BlackGenerationFunction>,

	/* BG2 function or name */
	///	__(Optional; PDF 1.3)__ Same as BG except that the value may also be the name
	///	Default , denoting the black-generation function that was in effect at the start
	///	of the page. If both BG and BG2 are present in the same graphics state param-
	///	eter dictionary, BG2 takes precedence.
	pub black_generation_extra: Option<BlackGenerationExtraFunction>,

	/* UCR function */
	///	__(Optional)__ The undercolor-removal function, which maps the interval
	///	[ 0.0 1.0 ] to the interval [ −1.0 1.0 ] (see Section 6.2.3, “Conversion from
	///	DeviceRGB to DeviceCMYK”).
	pub under_color_removal: Option<UnderColorRemovalFunction>,

	/* UCR2 function */
	///	__(Optional; PDF 1.3)__ Same as UCR except that the value may also be the name
	///	Default , denoting the undercolor-removal function that was in effect at the
	///	start of the page. If both UCR and UCR2 are present in the same graphics state
	///	parameter dictionary, UCR2 takes precedence.
	pub under_color_removal_extra: Option<UnderColorRemovalExtraFunction>,

	/* TR function */
	///	__(Optional)__ The transfer function, which maps the interval [ 0.0 1.0 ] to the in-
	///	terval [ 0.0 1.0 ] (see Section 6.3, “Transfer Functions”). The value is either a
	///	single function (which applies to all process colorants) or an array of four
	///	functions (which apply to the process colorants individually). The name
	///	Identity may be used to represent the identity function.
	pub transfer_function: Option<TransferFunction>,

	/* TR2 function */
	///	__(Optional; PDF 1.3)__ Same as TR except that the value may also be the name
	///	Default , denoting the transfer function that was in effect at the start of the
	///	page. If both TR and TR2 are present in the same graphics state parameter dic-
	///	tionary, TR2 takes precedence.
	pub transfer_extra_function: Option<TransferExtraFunction>,

	/* HT [dictionary, stream or name] */
	///	__(Optional)__ The halftone dictionary or stream (see Section 6.4, “Halftones”) or
	///	the name Default , denoting the halftone that was in effect at the start of the
	///	page.
	pub halftone_dictionary: Option<HalftoneType>,

	/* FL integer */
	///	__(Optional; PDF 1.3)__ The flatness tolerance (see Section 6.5.1, “Flatness Toler-
	///	ance”).
	pub flatness_tolerance: f64,

	/* SM integer */
	///	__(Optional; PDF 1.3)__ The smoothness tolerance (see Section 6.5.2, “Smooth-
	///	ness Tolerance”).
	pub smoothness_tolerance: f64,

	/* SA integer */
	///	(Optional) A flag specifying whether to apply automatic stroke adjustment
	///	(see Section 6.5.4, “Automatic Stroke Adjustment”).
	pub stroke_adjustment: bool,

	/* BM name or array */
	///	__(Optional; PDF 1.4)__ The current blend mode to be used in the transparent
	///	imaging model (see Sections 7.2.4, “Blend Mode,” and 7.5.2, “Specifying
	///	Blending Color Space and Blend Mode”).
	pub blend_mode: Vec<BlendMode>,

	/* SM dictionary or name */
	///	__(Optional; PDF 1.4)__ The current soft mask, specifying the mask shape or
	///	mask opacity values to be used in the transparent imaging model (see
	///	“Source Shape and Opacity” on page 526 and “Mask Shape and Opacity” on
	///	page 550).
	///
	///	*Note:* Although the current soft mask is sometimes referred to as a “soft clip,”
	///	altering it with the gs operator completely replaces the old value with the new
	///	one, rather than intersecting the two as is done with the current clipping path
	///	parameter (see Section 4.4.3, “Clipping Path Operators”).
	pub soft_mask: Option<SoftMask>,


	/* CA integer */
	///	__(Optional; PDF 1.4)__ The current stroking alpha constant, specifying the con-
	///	stant shape or constant opacity value to be used for stroking operations in the
	///	transparent imaging model (see “Source Shape and Opacity” on page 526 and
	///	“Constant Shape and Opacity” on page 551).
	pub current_stroke_alpha: f64,

	/* ca integer */
	/// __(Optional; PDF 1.4)__ Same as CA , but for nonstroking operations.
	pub current_fill_alpha: f64,

	/* AIS boolean */
	///	__(Optional; PDF 1.4)__ The alpha source flag (“alpha is shape”), specifying
	///	whether the current soft mask and alpha constant are to be interpreted as
	///	shape values ( true ) or opacity values ( false )
	/// true if the soft mask contains shape values, false for opacity
	pub alpha_is_shape: bool,

	/* TK boolean */
	///	__(Optional; PDF 1.4)__ The text knockout flag, which determines the behavior of
	///	overlapping glyphs within a text object in the transparent imaging model (see
	///	Section 5.2.7, “Text Knockout”).
	pub text_knockout: bool,
}

impl ExtendedGraphicsState {
	/// Creates a default ExtGState dictionary. Useful for resetting  
	pub fn default()
	-> Self
	{
		Self { .. Default::default() }
	}
}

impl Default for ExtendedGraphicsState {
	fn default()
	-> Self
	{
		Self {
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
			blend_mode: Vec::new(),
			soft_mask: None,
			current_stroke_alpha: 0.0,
			current_fill_alpha: 0.0,
			alpha_is_shape: false,
			text_knockout: false,
		}
	}
}

impl IntoPdfObject for ExtendedGraphicsState {
	// in this case the function will return the graphics state dictionary
	fn into_obj(self: Box<Self>)
	-> Vec<lopdf::Object>
	{
		use std::iter::FromIterator;
		vec![lopdf::Object::Dictionary(lopdf::Dictionary::from_iter(vec![
				("OP".to_string(), self.overprint_stroke.into()),
				("op".to_string(), self.overprint_fill.into()),
		]))]
	}
}

/// A reference to the graphics state, for reusing the 
/// graphics state during a stream without adding new graphics states all the time
pub struct ExtendedGraphicsStateRef {
	/// The name / hash of the graphics state
	pub(crate) gs_name: String,
}

impl ExtendedGraphicsStateRef {
	/// Creates a new graphics state reference (in order to be unique inside a page)
	#[inline]
	pub fn new(graphics_state_index: usize)
	-> Self
	{	
		Self {
			gs_name: format!("g{:?}", graphics_state_index)
		}
	}
}

/// __(PDF 1.3)__ A code specifying whether a color component value of 0
/// in a DeviceCMYK color space should erase that component (`EraseUnderlying`) or
/// leave it unchanged (`KeepUnderlying`) when overprinting (see Section 4.5.6, “Over-
/// print Control”). Initial value: `EraseUnderlying`
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum OverprintMode {
	/// Erase underlying color when overprinting
	EraseUnderlying,
	/// Keep underlying color when overprinting
	KeepUnderlying,
}

/// Black generation calculates the amount of black to be used when trying to 
/// reproduce a particular color.
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum BlackGenerationFunction {
	/// Regular black generation function
	/// ```rust,ignore
	/// let cyan = 1.0 - red;
	/// let magenta = 1.0 - green;
	/// let yellow = 1.0 - blue;
	/// let black = min(cyan, magenta, yellow);
	/// ```
	Default,
	/// Expects an UnderColorRemoval to be set. This will compensate
	/// the color for the added black
	/// ```rust,ignore
	/// let cyan = 1.0 - red;
	/// let magenta = 1.0 - green;
	/// let yellow = 1.0 - blue;
	/// let black = min(cyan, magenta, yellow);
	/// ```
	WithUnderColorRemoval,
}

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum BlackGenerationExtraFunction {

}

/// See `BlackGenerationFunction`, too. Undercolor removal reduces the amounts 
/// of the cyan, magenta, and yellow components to compensate for the amount of 
/// black that was added by black generation.
/// 
/// The undercolor-removal function computes the amount to subtract from each of 
/// the intermediate c, m, and y values to produce the final cyan, magenta, and yellow 
/// components. It can simply return its k operand unchanged, or it can return 0.0 
/// (so that no color is removed), some fraction of the black amount, or even a 
/// negative amount, thereby adding to the total amount of colorant.
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum UnderColorRemovalFunction {
	Default,
}

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum UnderColorRemovalExtraFunction {

}

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum TransferFunction {

}

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum TransferExtraFunction {

}

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
#[derive(Debug, PartialEq, Clone)]
pub enum HalftoneType {
	/// 1: Defines a single halftone screen by a frequency, angle, and spot function 
	Type1(f64, f64, SpotFunction),
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
	pub fn get_type(&self) 
	-> i64
	{
		use HalftoneType::*;
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
#[derive(Debug, PartialEq, Copy, Clone)]
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
	/// if (abs(x) + abs(y) <= 1 { 
	/// 	1 - (pow(x, 2) + pow(y, 2)) 
	/// } else { 
	/// 	pow((abs(x) - 1), 2) + pow((abs(y) - 1), 2) - 1 
	/// }
	/// ```
	Round,
	/// ```rust,ignore
	/// let w = (3 * abs(x)) + (4 * abs(y)) - 3;
	/// 
	/// if w < 0 { 
	/// 	1 - ((pow(x, 2) + pow((abs(y) / 0.75), 2)) / 4)
	/// } else if w > 1 { 
	/// 	pow((pow((1 - abs(x), 2) + (1 - abs(y)) / 0.75), 2) / 4) - 1
	/// } else {
	/// 	0.5 - w	
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
	/// 	1 - (pow(x, 2) + pow(y, 2))
	/// } else if t < 1.23 {
	/// 	1 - (0.85 * abs(x) + abs(y))
	/// } else {
	///		pow((abs(x) - 1), 2) + pow((abs(y) - 1), 2) - 1
	/// }
	/// ```
	Diamond,
}

impl IntoPdfObject for HalftoneType {
	fn into_obj(self: Box<Self>)
	-> Vec<lopdf::Object>
	{
		use std::iter::FromIterator;
		vec![lopdf::Object::Dictionary(lopdf::Dictionary::from_iter(vec![
					("Type", "Halftone".into()),
					("HalftoneType", self.get_type().into())
			]))]
	}
}

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum BlendMode {
	Normal,
	Compatible,
}


/* RI name (or ri inside a stream)*/
/// Although CIE-based color specifications are theoretically device-independent,
/// they are subject to practical limitations in the color reproduction capabilities of
/// the output device. Such limitations may sometimes require compromises to be
/// made among various properties of a color specification when rendering colors for
/// a given device. Specifying a rendering intent (PDF 1.1) allows a PDF file to set priorities 
/// regarding which of these properties to preserve and which to sacrifice.
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum RenderingIntent {
	///	Colors are represented solely with respect to the light source; no
	///	correction is made for the output medium’s white point (such as
	///	the color of unprinted paper). Thus, for example, a monitor’s
	///	white point, which is bluish compared to that of a printer’s paper,
	/// would be reproduced with a blue cast. In-gamut colors are
	///	reproduced exactly; out-of-gamut colors are mapped to the
	///	nearest value within the reproducible gamut. This style of reproduction 
	/// has the advantage of providing exact color matches
	///	from one output medium to another. It has the disadvantage of
	///	causing colors with Y values between the medium’s white point
	///	and 1.0 to be out of gamut. A typical use might be for logos and
	///	solid colors that require exact reproduction across different media.
	AbsoluteColorimetric,
	///	Colors are represented with respect to the combination of the
	///	light source and the output medium’s white point (such as the
	///	color of unprinted paper). Thus, for example, a monitor’s white
	///	point would be reproduced on a printer by simply leaving the
	///	paper unmarked, ignoring color differences between the two
	///	media. In-gamut colors are reproduced exactly; out-of-gamut
	///	colors are mapped to the nearest value within the reproducible
	///	gamut. This style of reproduction has the advantage of adapting
	///	for the varying white points of different output media. It has the
	///	disadvantage of not providing exact color matches from one me-
	///	dium to another. A typical use might be for vector graphics.
	RelativeColorimetric,
	///	Colors are represented in a manner that preserves or emphasizes
	///	saturation. Reproduction of in-gamut colors may or may not be
	///	colorimetrically accurate. A typical use might be for business
	///	graphics, where saturation is the most important attribute of the
	///	color.
	Saturation,
	///	Colors are represented in a manner that provides a pleasing perceptual 
	/// appearance. To preserve color relationships, both in-gamut 
	/// and out-of-gamut colors are generally modified from
	///	their precise colorimetric values. A typical use might be for scanned images.
	Perceptual,
}

/* ri name */
impl IntoPdfStreamOperation for RenderingIntent {
	fn into_stream_op(self: Box<Self>)
	-> Vec<Operation>
	{
		use RenderingIntent::*;
		let rendering_intent_string = match *self {
			AbsoluteColorimetric => "AbsoluteColorimetric",
			RelativeColorimetric => "RelativeColorimetric",
			Saturation => "Saturation",
			Perceptual => "Perceptual",
		};

		vec![Operation::new("ri", vec![Name(rendering_intent_string.as_bytes().to_vec())] )]
	}
}

/* RI name , only to be used in graphics state dictionary */
impl IntoPdfObject for RenderingIntent {
    /// Consumes the object and converts it to an PDF object
    fn into_obj(self: Box<Self>)
    -> Vec<lopdf::Object>
    {
    	use RenderingIntent::*;
    	let rendering_intent_string = match *self {
    		AbsoluteColorimetric => "AbsoluteColorimetric",
    		RelativeColorimetric => "RelativeColorimetric",
    		Saturation => "Saturation",
    		Perceptual => "Perceptual",
    	};

    	vec![Name(b"RI".to_vec()), Name(rendering_intent_string.as_bytes().to_vec())]
    }
}

/// A soft mask is used for transparent images such as PNG with an alpha component
/// The bytes range from 0xFF (opaque) to 0x00 (transparent). The alpha channel of a
/// PNG image have to be sorted out.
/// Can also be used for Vignettes, etc.
/// Beware of color spaces! 
/// __See PDF Reference Page 545__ - Soft masks
#[derive(Debug, PartialEq, Clone)]
pub struct SoftMask {
	/// The data to be used as a soft mask
	data: Vec<u8>,
	/// Bits per component (1 for black / white, 8 for greyscale, up to 16)
	bits_per_component: u8,
}

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum SoftMaskFunction {
	// (Color, Shape, Alpha) = Composite(Color0, Alpha0, Group)
	/// In this function, the backdrop color does not contribute to the result.
	/// This is the easies function, but may look bad at edges.
	GroupAlpha,
	// 
	GroupLuminosity,

}
/// __See PDF Reference Page 216__ - Line join style
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum LineJoinStyle {
	///	Miter join. The outer edges of the strokes for the two segments are extended
	///	until they meet at an angle, as in a picture frame. If the segments meet at too
	///	sharp an angle (as defined by the miter limit parameter—see “Miter Limit,”
	///	above), a bevel join is used instead.
	Miter,
	///	Round join. An arc of a circle with a diameter equal to the line width is drawn
	///	around the point where the two segments meet, connecting the outer edges of
	///	the strokes for the two segments. This pieslice-shaped figure is filled in, pro-
	///	ducing a rounded corner.
	Round,
	///	Bevel join. The two segments are finished with butt caps (see “Line Cap Style”
	///	on page 216) and the resulting notch beyond the ends of the segments is filled
	///	with a triangle.
	Limit,
}

impl IntoPdfStreamOperation for LineJoinStyle {
    fn into_stream_op(self: Box<Self>)
    -> Vec<Operation>
    {
    	use LineJoinStyle::*;
    	let line_join_num = match *self {
    		Miter => 0,
    		Round => 1,
    		Limit => 2,
    	};

    	vec![Operation::new("j", vec![Integer(line_join_num)])]
    }
}

/// __See PDF Reference (Page 216)__ - Line cap (ending) style
#[derive(Debug, PartialEq, Copy, Clone)]
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

impl IntoPdfStreamOperation for LineCapStyle {
    fn into_stream_op(self: Box<Self>)
    -> Vec<Operation>
    {
    	use LineCapStyle::*;
    	let line_cap_num = match *self {
    		Butt => 0,
    		Round => 1,
    		ProjectingSquare => 2,
    	};

    	vec![Operation::new("J", vec![Integer(line_cap_num)])]
    }
}

/// Line dash pattern is made up of a total width
#[derive(Debug, PartialEq, Copy, Clone)]
pub struct LineDashPattern {
	/// Offset at which the dashing pattern should start, measured from the beginning ot the line
	/// Default: 0 (start directly where the line starts)
	pub offset: i64,
	/// Length of the first dash in the dash pattern. If `None`, the line will be solid (good for resetting the dash pattern)
	pub dash_1: Option<i64>,
	/// Whitespace after the first dash. If `None`, whitespace will be the same as length_1st, 
	/// meaning that the line will have dash - whitespace - dash - whitespace in even offsets
	pub gap_1: Option<i64>,
	/// Length of the second dash in the dash pattern. If None, will be equal to length_1st
	pub dash_2: Option<i64>,
	/// Same as whitespace_1st, but for length_2nd
	pub gap_2: Option<i64>,
	/// Length of the second dash in the dash pattern. If None, will be equal to length_1st
	pub dash_3: Option<i64>,
	/// Same as whitespace_1st, but for length_3rd
	pub gap_3: Option<i64>,
}

impl LineDashPattern {
	/// Creates a new dash pattern
	pub fn new(offset: i64, dash_1: Option<i64>, gap_1: Option<i64>, dash_2: Option<i64>, gap_2: Option<i64>, dash_3: Option<i64>, gap_3: Option<i64>)
	-> Self
	{
		Self { offset, dash_1, gap_1, dash_2, gap_2, dash_3, gap_3 }
	}

	/// Creates a new dash pattern
	pub fn default()
	-> Self
	{
		Self { offset: 0, dash_1: None, gap_1: None, dash_2: None, gap_2: None, dash_3: None, gap_3: None }
	}
}

impl IntoPdfStreamOperation for LineDashPattern {
    fn into_stream_op(self: Box<Self>)
    -> Vec<Operation>
    {
    	let mut dash_array = Vec::<i64>::new();
    	
    	// note: it may be that PDF allows more than 6 operators. 
    	// I've not seen it in practise, though

    	// break as soon as we encounter a None
    	loop {

	    	if let Some(d1) = self.dash_1 {
	    		dash_array.push(d1);
	    	} else { break; }

	    	if let Some(g1) = self.gap_1 {
	    		dash_array.push(g1);
	    	} else { break; }

	    	if let Some(d2) = self.dash_2 {
	    		dash_array.push(d2);
	    	} else { break; }

	    	if let Some(g2) = self.gap_2 {
	    		dash_array.push(g2);
	    	} else { break; }

	    	if let Some(d3) = self.dash_3 {
	    		dash_array.push(d3);
	    	} else { break; }

	    	if let Some(g3) = self.gap_3 {
	    		dash_array.push(g3);
	    	} else { break; }

	    	break;
    	}

    	let dash_array_ints = dash_array.into_iter().map(|int| Integer(int)).collect();

    	vec![Operation::new("d", vec![Array(dash_array_ints), Integer(self.offset)])]
    }
}