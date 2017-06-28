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

/// Line dash pattern is made up of a total width
#[derive(Debug)]
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