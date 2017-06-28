use lopdf;
use traits::IntoPdfStreamOperation;

#[derive(Debug)]
pub struct CurrentTransformationMatrix {
	pub translate_x: f64,
	pub translate_y: f64,
	pub scale_x: f64,
	pub scale_y: f64,
	pub rotation_ccw_angle: f64,
}

impl CurrentTransformationMatrix {
	/// Creates a new transformation matrix
	pub fn new(translate_x: f64, translate_y: f64, scale_x: f64, scale_y: f64, rotation_ccw_angle: f64)
	-> Self
	{
		Self {
			translate_x,
			translate_y,
			scale_x,
			scale_y,
			rotation_ccw_angle,
		}
	}

	/// Returns a default CTM that does nothing.
	pub fn default()
	-> Self
	{
		Self {
			translate_x: 0.0,
			translate_y: 0.0,
			scale_x: 1.0,
			scale_y: 1.0,
			rotation_ccw_angle: 0.0,
		}
	}
}

impl IntoPdfStreamOperation for CurrentTransformationMatrix {
	/// Consumes the object and converts it to an PDF stream operation
	fn into_stream_op(self: Box<Self>)
	-> Vec<lopdf::content::Operation>
	{
		use lopdf::Object::*;
		let rotation_rad = self.rotation_ccw_angle.to_radians();

		let cos_x = rotation_rad.cos();
		let sin_x = rotation_rad.sin();

		vec![lopdf::content::Operation::new("cm", vec![
			Real(self.scale_x + cos_x), Real(sin_x), Real(-sin_x), Real(self.scale_y + cos_x), Real(self.translate_x), Real(self.translate_y)])]
	}
}