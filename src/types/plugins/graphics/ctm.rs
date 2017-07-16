//! Current transformation matrix, for transforming shapes (rotate, translate, scale)

use lopdf;
use lopdf::content::Operation;

/// PDF "current transformation matrix". Once set, will operate on all following shapes,
/// until the `layer.restore_graphics_state()` is called. It is important to 
/// call `layer.save_graphics_state()` earlier.
#[derive(Debug, Clone)]
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
    Identity,

    /// Text rotation matrix. Text placement is a bit different, but uses the same
    /// basic concepts as a CTM that's why it's merged here
    TextRotate(f64),
    /// Text scaling matrix, used for spacing characters 
    TextScale(f64, f64),
    /// Text translate matrix, used for indenting (transforming) text 
    /// (different to regular text placement)
    TextTranslate(f64, f64),
    /// Text identity matrix. For completeness, may be useful
    TextIdentity,
}

impl Into<[f64; 6]> for CurTransMat {
    fn into(self)
    -> [f64; 6]
    {
        use CurTransMat::*;
        match self {
            Translate(x, y) | TextTranslate(x, y) => { [ 1.0, 0.0, 0.0, 1.0, mm_to_pt!(x), mm_to_pt!(y) ]  /* 1 0 0 1 x y cm */ }
            Rotate(rot) | TextRotate(rot) => { let rad = (360.0 - rot).to_radians(); [rad.cos(), -rad.sin(), rad.sin(), rad.cos(), 0.0, 0.0 ] /* cos sin -sin cos 0 0 cm */ }
            Scale(x, y) | TextScale(x, y) => { [ x, 0.0, 0.0, y, 0.0, 0.0 ] /* x 0 0 y 0 0 cm */ }
            Identity | TextIdentity => { [ 1.0, 0.0, 0.0, 1.0, 0.0, 0.0 ] }
        }
    }
}

impl Into<Operation> for CurTransMat {
	
	/// Consumes the object and converts it to an PDF stream operation
	fn into(self)
	-> Operation
	{
		use lopdf::Object::*;
        let matrix_nums: [f64; 6] = self.clone().into();
        let matrix: Vec<lopdf::Object> = matrix_nums.to_vec().into_iter().map(|float| Real(float)).collect();

        use CurTransMat::*;

        match self {
            // text matrices have different operators
            TextTranslate(_, _) | TextRotate(_) |
            TextScale(_, _)     | TextIdentity   => Operation::new("Tm", matrix),
            
            // regular matrix
            _ => Operation::new("cm", matrix),
        }
		
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