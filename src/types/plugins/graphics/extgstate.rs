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
//! new_state.overprint_stroke = true;
//! 
//! // it is best to put the next lines in a seperate function
//! // A PdfLayerReferences contains the indices of the page and the layer 
//! // as well as a `std::sync::Weak` reference to the document.
//! // This is why you need the braces, otherwise, you'll trigger a deadlock
//! {
//! 	// supposing mylayer is a PdfLayerReference
//!     let doc = mylayer.document.upgrade().unwrap();
//! 	let mut doc = doc.lock().unwrap();
//! 	let mut page = doc.pages.get_mut(self.page.0).unwrap();
//!
//! 	// see the documentation for add_graphics_state
//! 	page.add_graphics_state(new_state);
//! }
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
	pub blend_mode: BlendMode,

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
			blend_mode: BlendMode::Seperable(SeperableBlendMode::Normal),
			soft_mask: None,
			current_stroke_alpha: 1.0, /* 1.0 = opaque, not transparent*/
			current_fill_alpha: 1.0,
			alpha_is_shape: false,
			text_knockout: false,
		}
	}
}

impl Into<lopdf::Object> for ExtendedGraphicsState {
	// in this case the function will return the graphics state dictionary
	fn into(self)
	-> lopdf::Object
	{
		use std::iter::FromIterator;
		let mut graphics_state = lopdf::Dictionary::from_iter(vec![
				("Type".to_string(), "ExtGState".into()),
				("LW".to_string(), self.line_width.into()), 
				("LC".to_string(), self.line_cap.into()),
				("LJ".to_string(), self.line_join.into()),
				("ML".to_string(), self.miter_limit.into()), 
				("FL".to_string(), self.flatness_tolerance.into()),
				("RI".to_string(), self.rendering_intent.into()),
				("SA".to_string(), self.stroke_adjustment.into()),
				("OP".to_string(), self.overprint_fill.into()),
				("OPM".to_string(), self.overprint_mode.into()),
				("op".to_string(), self.overprint_stroke.into()),
				("CA".to_string(), self.current_fill_alpha.into()), 
				("ca".to_string(), self.current_stroke_alpha.into()), 
				("BM".to_string(), self.blend_mode.into()),
				("AIS".to_string(), self.alpha_is_shape.into()), 
				("TK".to_string(), self.text_knockout.into()),
		]);

		// set optional parameters
		if let &Some(ldp) = &self.line_dash_pattern {
			let pattern: lopdf::Object = ldp.into();
			graphics_state.set("D".to_string(), pattern);
		}

		if let &Some(ref font) = &self.font {
			
		}

		if let &Some(ref black_generation) = &self.black_generation {
			
		}

		if let &Some(ref black_generation_extra) = &self.black_generation_extra {
			
		}

		if let &Some(ref under_color_removal) = &self.under_color_removal {
			
		}

		if let &Some(ref under_color_removal_extra) = &self.under_color_removal_extra {
			
		}

		if let &Some(ref transfer_function) = &self.transfer_function {
			
		}

		if let &Some(ref transfer_extra_function) = &self.transfer_extra_function {
			
		}

		if let &Some(ref halftone_dictionary) = &self.halftone_dictionary {
			
		}

		if let &Some(ref soft_mask) = &self.soft_mask {
			
		} else {
			graphics_state.set("SM".to_string(), lopdf::Object::Name("None".as_bytes().to_vec()));
		}

		return lopdf::Object::Dictionary(graphics_state);
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
			gs_name: format!("GS{:?}", graphics_state_index)
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
	EraseUnderlying, /* 0, default */
	/// Keep underlying color when overprinting
	KeepUnderlying,  /* 1 */
}

impl Into<lopdf::Object> for OverprintMode {
	fn into(self)
	-> lopdf::Object
	{
		use OverprintMode::*;
		match self {
			EraseUnderlying		=> lopdf::Object::Integer(0),
			KeepUnderlying 		=> lopdf::Object::Integer(1),
		}
	}
}

/// Black generation calculates the amount of black to be used when trying to 
/// reproduce a particular color.
#[derive(Debug, PartialEq, Copy, Clone)]
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
	Seperable(SeperableBlendMode),
	NonSeperable(NonSeperableBlendMode),
}

impl Into<lopdf::Object> for BlendMode {
	fn into(self)
	-> lopdf::Object {
		use BlendMode::*;
		use SeperableBlendMode::*;
		use NonSeperableBlendMode::*;

		let blend_mode_str = match self {
			Seperable(s) => {
				match s {
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
				}
			},
			NonSeperable(n) => {
				match n {
					Hue => "Hue",
					Saturation => "Saturation",
					Color => "Color",
					Luminosity => "Luminosity",
				}
			}
		};

		lopdf::Object::Name(blend_mode_str.as_bytes().to_vec())
	}
}

/// PDF Reference 1.7, Page 520, Table 7.2
/// Blending modes for objects
/// In the following reference, each function gets one new color (the thing to paint on top)
/// and an old color (the color that was already present before the object gets painted)
/// 
/// The function simply notes the formula that has to be applied to (color_new, color_old) in order
/// to get the desired effect. You have to run each formula once for each color channel.
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum SeperableBlendMode {
	/// Selects the source color, ignoring the old color. Default mode.
	///
	/// `color_new`
	Normal,
	/// Multiplies the old color and source color values
	/// Note that these values have to be in the range [0.0 to 1.0] to work.
	///	The result color is always at least as dark as either of the two constituent 
	///	colors. Multiplying any color with black produces black; multiplying with white 
	///	leaves the original color unchanged.Painting successive overlapping objects with 
	/// a color other than black or white produces progressively darker colors.
	///
	/// `color_old * color_new`
	Multiply,
	/// Multiplies the complements of the old color and new color values, then 
	/// complements the result
	///	The result color is always at least as light as either of the two constituent colors. 
	///	Screening any color with white produces white; screening with black leaves the original 
	///	color unchanged. The effect is similar to projecting multiple photographic slides 
	///	simultaneously onto a single screen.
	///
	/// `color_old + color_new - (color_old * color_new)`
	Screen,
	///	Multiplies or screens the colors, depending on the old color value. Source colors 
	///	overlay the old color while preserving its highlights and shadows. The old color is 
	///	not replaced but is mixed with the source color to reflect the lightness or darkness 
	///	of the old color.
	///
	/// TLDR: It's the inverse of HardLight
	///
	/// ```rust,ignore
	/// if color_old <= 0.5 {
	/// 	Multiply(color_new, 2 x color_old)
	/// } else {
	/// 	Screen(color_new, 2 * color_old - 1)
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
	/// 	min(1, color_old / (1 - color_new))
	/// } else {
	///		1
	/// }
	/// ```
	ColorDodge,
	/// Darkens the backdrop color to reflect the source color. Painting with 
	/// white produces no change.
	///
	/// ```rust,ignore
	/// if color_new > 0 {
	/// 	1 - min(1, (1 - color_old) / color_new))
	/// } else {
	///		0
	/// }
	/// ```
	ColorBurn,
	/// Multiplies or screens the colors, depending on the source color value. The effect is 
	/// similar to shining a harsh spotlight on the old color. It's the inverse of Screen.
	///
	/// ```rust,ignore
	/// if color_new <= 0.5 {
	/// 	Multiply(color_old, 2 x color_new)
	/// } else {
	/// 	Screen(color_old, 2 * color_new - 1)
	/// }
	/// ```
	HardLight,
	/// Darkens or lightens the colors, depending on the source color value. 
	/// The effect is similar to shining a diffused spotlight on the backdrop.
	///
	/// ```rust,ignore
	/// if color_new <= 0.5 {
	/// 	color_old - ((1 - (2 * color_new)) * color_old * (1 - color_old))
	/// } else {
	/// 	let mut dx_factor = color_old.sqrt();
	///		if color_old <= 0.25 {
	///			dx_factor = (((16 * color_old - 12) * color_old) + 4) * color_old;
	///		}
	///		color_old + ((2 * color_new) - 1) * (dx_factor - color_old)
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
/// 1. Convert the backdrop and source colors from the blending color space to an intermediate 
///    HSL (hue-saturation-luminosity) representation.
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
/// ```rust
/// # #[macro_use] extern crate printpdf;
/// # use printpdf::Rgb;
/// # use printpdf::glob_macros::*;
/// # fn main() { /* needed for testing*/ }
/// fn luminosity(input: Rgb) -> f64 {
/// 	0.3 * input.r + 0.59 * input.g + 0.11 * input.b
/// }
/// 
/// fn set_luminosity(input: Rgb, target_luminosity: f64) -> Rgb {
/// 	let d = target_luminosity - luminosity(input);
/// 	Rgb {
/// 		r: input.r + d,
/// 		g: input.g + d,
/// 		b: input.b + d,
/// 		icc_profile: input.icc_profile,
/// 	}
/// }
/// 
/// fn clip_color(mut input: Rgb) -> Rgb {
/// 
/// 	let lum = luminosity(input);
/// 
/// 	let mut cur_r = (input.r * 1000.0) as i64;
/// 	let mut cur_g = (input.g * 1000.0) as i64;
/// 	let mut cur_b = (input.b * 1000.0) as i64;
///
/// 	/// min! and max! is defined in printpdf/src/glob_macros.rs
/// 	let mut min = min!(cur_r, cur_g, cur_b);
/// 	let mut max = max!(cur_r, cur_g, cur_b);
///  
///		let new_min = (min as f64) / 1000.0; 
///		let new_max = (max as f64) / 1000.0;
///	
/// 	if new_min < 0.0 { 
/// 		input.r = lum + (((input.r - lum) * lum) / (lum - new_min));
/// 		input.g = lum + (((input.g - lum) * lum) / (lum - new_min));
/// 		input.b = lum + (((input.b - lum) * lum) / (lum - new_min));
/// 	} else if new_max > 1.0 {
/// 		input.r = lum + ((input.r - lum) * (1.0 - lum) / (new_max - lum));
/// 		input.g = lum + ((input.g - lum) * (1.0 - lum) / (new_max - lum));
/// 		input.b = lum + ((input.b - lum) * (1.0 - lum) / (new_max - lum));
/// 	}
/// 
/// 	return input;
/// }
/// 
/// fn saturation(input: Rgb) -> f64 {
/// 	let mut cur_r = (input.r * 1000.0) as i64;
/// 	let mut cur_g = (input.g * 1000.0) as i64;
/// 	let mut cur_b = (input.b * 1000.0) as i64;
///
/// 	/// min! and max! is defined in printpdf/src/glob_macros.rs
/// 	let mut min = min!(cur_r, cur_g, cur_b);
/// 	let mut max = max!(cur_r, cur_g, cur_b);
///  
///		let new_min = (min as f64) / 1000.0;
///		let new_max = (max as f64) / 1000.0;
/// 	new_max - new_min
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
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum NonSeperableBlendMode {
	Hue,
	Saturation,
	Color,
	Luminosity,
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
impl Into<lopdf::Object> for RenderingIntent {
    /// Consumes the object and converts it to an PDF object
    fn into(self)
    -> lopdf::Object
    {
    	use RenderingIntent::*;
    	let rendering_intent_string = match self {
    		AbsoluteColorimetric => "AbsoluteColorimetric",
    		RelativeColorimetric => "RelativeColorimetric",
    		Saturation => "Saturation",
    		Perceptual => "Perceptual",
    	};

    	lopdf::Object::Name(rendering_intent_string.as_bytes().to_vec())
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
	/// In this function, the old (backdrop) color does not contribute to the result.
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

impl Into<i64> for LineJoinStyle {
	fn into(self)
	-> i64
	{
		use LineJoinStyle::*;
		match self {
			Miter => 0,
			Round => 1,
			Limit => 2,
		}
	}
}

impl IntoPdfStreamOperation for LineJoinStyle {
    fn into_stream_op(self: Box<Self>)
    -> Vec<Operation>
    {
    	let data = *self;
    	let line_join_num: i64 = data.into();
    	vec![Operation::new("j", vec![Integer(line_join_num)])]
    }
}

impl Into<lopdf::Object> for LineJoinStyle {
	fn into(self)
	-> lopdf::Object 
	{
		lopdf::Object::Integer(self.into())
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

impl Into<i64> for LineCapStyle {
	fn into(self)
	-> i64
	{
		use LineCapStyle::*;
		match self {
			Butt => 0,
			Round => 1,
			ProjectingSquare => 2,
		}
	}
}

impl IntoPdfStreamOperation for LineCapStyle {
    fn into_stream_op(self: Box<Self>)
    -> Vec<Operation>
    {
    	let data = *self;
    	vec![Operation::new("J", vec![Integer(data.into())])]
    }
}

impl Into<lopdf::Object> for LineCapStyle {
	fn into(self)
	-> lopdf::Object 
	{
		lopdf::Object::Integer(self.into())
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

// conversion into a dash array for reuse in operation / gs dictionary
impl Into<(Vec<i64>, i64)> for LineDashPattern {
	fn into(self)
	-> (Vec<i64>, i64)
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

		return (dash_array, self.offset);
	}
	
}

impl Into<Operation> for LineDashPattern {
    fn into(self)
    -> Operation
    {
    	let (dash_array, offset) = self.into();
    	let dash_array_ints = dash_array.into_iter().map(|int| Integer(int)).collect();
    	Operation::new("d", vec![Array(dash_array_ints), Integer(offset)])
    }
}

impl Into<lopdf::Object> for LineDashPattern {
	fn into(self)
	-> lopdf::Object
	{
		use lopdf::Object::*;
		let (dash_array, offset) = self.into();
		let mut dash_array_ints: Vec<lopdf::Object> = dash_array.into_iter().map(|int| Integer(int)).collect();
		dash_array_ints.push(Integer(offset));
		Array(dash_array_ints)
	}
}