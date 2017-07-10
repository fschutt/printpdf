//! Current transformation matrix, for transforming shapes (rotate, translate, scale)

use lopdf;
use traits::IntoPdfStreamOperation;

/// PDF "current transformation matrix". Once set, will operate on all following shapes,
/// until the `layer.restore_graphics_state()` is called. It is important to 
/// call `layer.save_graphics_state()` earlier.
#[derive(Debug)]
pub enum CurTransMat {
    /// Translation matrix (in millimeter from bottom left corner)
    /// X and Y can have different values
    Translate(f64, f64),
    /// Rotation matrix (clockwise, in degrees)
    Rotate(f64),
    /// Scale matrix (1.0 = 100% scale, no change)
    /// X and Y can have different values
    Scale(f64, f64),
    /// Identity matrix
    Identity
}

impl CurTransMat {

    /// Creates a translation matrix
    #[inline]
    pub fn translate(x: f64, y: f64)
    -> Self {
        CurTransMat::Translate(x, y)
    }

    /// Returns a rotation matrix 
    /// Input: rotation (clockwise) in degrees
    #[inline]
    pub fn rotate(rot: f64)
    -> Self {
        CurTransMat::Rotate(rot)
    }

    /// Returns a scaling matrix
    #[inline]
    pub fn scale(x: f64, y: f64)
    -> Self {
        CurTransMat::Scale(x, y)
    }

	/// Returns a default CTM that does nothing.
    /// Also called "identity" matrix
    #[inline]
	pub fn identity()
	-> Self
	{
		CurTransMat::Identity
	}
}

impl Into<[f64; 6]> for CurTransMat {
    fn into(self)
    -> [f64; 6]
    {
        use CurTransMat::*;
        match self {
            Translate(x, y) => { [ 1.0, 0.0, 0.0, 1.0, mm_to_pt!(x), mm_to_pt!(y) ]  /* 1 0 0 1 x y cm */ }
            Rotate(rot) => { let rad = (360.0 - rot).to_radians(); [rad.cos(), -rad.sin(), rad.sin(), rad.cos(), 0.0, 0.0 ] /* cos sin -sin cos 0 0 cm */ }
            Scale(x, y) => { [ x, 0.0, 0.0, y, 0.0, 0.0 ] /* x 0 0 y 0 0 cm */ }
            Identity => { [ 1.0, 0.0, 0.0, 1.0, 0.0, 0.0 ] }
        }
    }
}


impl IntoPdfStreamOperation for CurTransMat {
	
	/// Consumes the object and converts it to an PDF stream operation
	fn into_stream_op(self: Box<Self>)
	-> Vec<lopdf::content::Operation>
	{
		use lopdf::Object::*;
        let s = *self;
        let matrix_nums: [f64; 6] = s.into();
        let matrix: Vec<lopdf::Object> = matrix_nums.to_vec().into_iter().map(|float| Real(float)).collect();

		vec![lopdf::content::Operation::new("cm", matrix)]
	}
}

impl Into<lopdf::Object> for CurTransMat {
    fn into(self)
    -> lopdf::Object
    {
        use lopdf::Object::*;
        let matrix_nums: [f64; 6] = self.into();
        Array(matrix_nums.to_vec().into_iter().map(|float| Real(float)).collect())
    }
}

#[test]
fn test_ctm_translate()
{
    use self::*;

    // test that the translation matrix look like what PDF expects
    let ctm_trans = CurTransMat::translate(150.0, 50.0);
    let ctm_trans_arr: [f64; 6] = ctm_trans.into();
    assert_eq!([1.0_f64, 0.0, 0.0, 1.0, 425.1969, 141.7323], ctm_trans_arr);

    let ctm_scale = CurTransMat::scale(2.0, 4.0);
    let ctm_scale_arr: [f64; 6] = ctm_scale.into();
    assert_eq!([2.0_f64, 0.0, 0.0, 4.0, 0.0, 0.0], ctm_scale_arr);

    let ctm_rot = CurTransMat::rotate(30.0);
    let ctm_rot_arr: [f64; 6] = ctm_rot.into();
    assert_eq!([0.8660254037844384, 0.5000000000000004, -0.5000000000000004, 0.8660254037844384, 0.0, 0.0], ctm_rot_arr);
}