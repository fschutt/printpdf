//! Module for graphics (line, polygon, 3D, etc.)

pub mod two_dimensional;
pub mod three_dimensional;

pub use self::two_dimensional::*;
pub use self::three_dimensional::*;

pub mod outline;
pub mod fill;
pub mod style;
pub mod color;
pub mod icc_profile;
pub mod ctm;

pub use self::style::Style;
pub use self::outline::Outline;
pub use self::ctm::CurrentTransformationMatrix;
pub use self::fill::Fill;
pub use self::color::{Color, Rgb, Cmyk, Grayscale};
pub use self::icc_profile::{IccProfile, IccProfileType};

// simple enums that don't have to go into their own file
use lopdf::content::Operation;
use lopdf::Object::*;
use traits::IntoPdfStreamOperation;

/// Line join style
#[derive(Debug)]
pub enum LineJoinStyle {
	Miter,
	Round,
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

/// Line cap (ending) style
#[derive(Debug)]
pub enum LineCapStyle {
	Butt,
	Round,
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

pub struct LineDashStyle {
	/// Offset at which the dashing pattern should start, measured from the beginning ot the line
	/// Default: 0 (start directly where the line starts)
	offset: u32,
	/// Length of the first dash in the dash pattern
	dash_1: u32,
	/// Whitespace after the first dash. If `None`, whitespace will be the same as length_1st, 
	/// meaning that the line will have dash - whitespace - dash - whitespace in even offsets
	gap_1: Option<u32>,
	/// Length of the second dash in the dash pattern. If None, will be equal to length_1st
	dash_2: Option<u32>,
	/// Same as whitespace_1st, but for length_2nd
	gap_2: Option<u32>,
	/// Length of the second dash in the dash pattern. If None, will be equal to length_1st
	dash_3: Option<u32>,
	/// Same as whitespace_1st, but for length_3rd
	gap_3: Option<u32>,
}
