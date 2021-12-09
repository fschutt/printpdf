//! Current transformation matrix, for transforming shapes (rotate, translate, scale)

use lopdf;
use lopdf::content::Operation;
use {Mm, Pt};

/// PDF "current transformation matrix". Once set, will operate on all following shapes,
/// until the `layer.restore_graphics_state()` is called. It is important to
/// call `layer.save_graphics_state()` earlier.
#[derive(Debug, Copy, Clone)]
pub enum CurTransMat {
    /// Translation matrix (in points from bottom left corner)
    /// X and Y can have different values
    Translate(Mm, Mm),
    /// Rotation matrix (clockwise, in degrees)
    Rotate(f64),
    /// Scale matrix (1.0 = 100% scale, no change)
    /// X and Y can have different values
    Scale(f64, f64),
    /// Identity matrix
    Identity,
}

/// Text matrix. Text placement is a bit different, but uses the same
/// concepts as a CTM that's why it's merged here
///
/// Note: `TextScale` does not exist. Use `layer.set_word_spacing()`
/// and `layer.set_character_spacing()` to specify the scaling between words
/// and characters.
#[derive(Debug, Copy, Clone)]
pub enum TextMatrix {
    /// Text rotation matrix, used for rotating text
    Rotate(f64),
    /// Text translate matrix, used for indenting (transforming) text
    /// (different to regular text placement)
    Translate(Mm, Mm),
    /// Combined translate + rotate matrix
    TranslateRotate(Mm, Mm, f64),
}

impl Into<[f64; 6]> for TextMatrix {
    fn into(self)
    -> [f64; 6]
    {
        use TextMatrix::*;
        match self {
            Translate(x, y) => { 
                // 1 0 0 1 x y cm 
                let x_pt: Pt = x.into();
                let y_pt: Pt = y.into();
                [ 1.0, 0.0, 0.0, 1.0, x_pt.0, y_pt.0 ] 
            }
            Rotate(rot) => {
                let rad = (360.0 - rot).to_radians();
                [rad.cos(), -rad.sin(), rad.sin(), rad.cos(), 0.0, 0.0 ] /* cos sin -sin cos 0 0 cm */
            },
            TranslateRotate(x, y, rot) => {
                let rad = (360.0 - rot).to_radians();
                let x_pt: Pt = x.into();
                let y_pt: Pt = y.into();
                [rad.cos(), -rad.sin(), rad.sin(), rad.cos(), x_pt.0, y_pt.0 ] /* cos sin -sin cos x y cm */
            }
        }
    }
}

impl Into<[f64; 6]> for CurTransMat {
    fn into(self)
    -> [f64; 6]
    {
        use CurTransMat::*;
        match self {
            Translate(x, y) => { 
                // 1 0 0 1 x y cm 
                let x_pt: Pt = x.into();
                let y_pt: Pt = y.into();
                [ 1.0, 0.0, 0.0, 1.0, x_pt.0, y_pt.0 ]   
            }
            Rotate(rot) => { 
                // cos sin -sin cos 0 0 cm 
                let rad = (360.0 - rot).to_radians(); 
                [rad.cos(), -rad.sin(), rad.sin(), rad.cos(), 0.0, 0.0 ] 
            }
            Scale(x, y) => { 
                // x 0 0 y 0 0 cm
                [ x, 0.0, 0.0, y, 0.0, 0.0 ] 
            }
            Identity => { 
                [ 1.0, 0.0, 0.0, 1.0, 0.0, 0.0 ] 
            }
        }
    }
}

impl Into<Operation> for CurTransMat {
	fn into(self)
	-> Operation
	{
		use lopdf::Object::*;
        let matrix_nums: [f64; 6] = self.into();
        let matrix: Vec<lopdf::Object> = matrix_nums.to_vec().into_iter().map(Real).collect();
        Operation::new("cm", matrix)
	}
}

impl Into<Operation> for TextMatrix {
    fn into(self)
    -> Operation
    {
        use lopdf::Object::*;
        let matrix_nums: [f64; 6] = self.into();
        let matrix: Vec<lopdf::Object> = matrix_nums.to_vec().into_iter().map(Real).collect();
        Operation::new("Tm", matrix)
    }
}

impl Into<lopdf::Object> for CurTransMat {
    fn into(self)
    -> lopdf::Object
    {
        use lopdf::Object::*;
        let matrix_nums: [f64; 6] = self.into();
        Array(matrix_nums.to_vec().into_iter().map(Real).collect())
    }
}

#[test]
fn test_ctm_translate()
{
    use self::*;

    // test that the translation matrix look like what PDF expects
    let ctm_trans = CurTransMat::Translate(Mm(150.0), Mm(50.0));
    let ctm_trans_arr: [f64; 6] = ctm_trans.into();
    assert_eq!([1.0_f64, 0.0, 0.0, 1.0, 425.1969, 141.7323], ctm_trans_arr);

    let ctm_scale = CurTransMat::Scale(2.0, 4.0);
    let ctm_scale_arr: [f64; 6] = ctm_scale.into();
    assert_eq!([2.0_f64, 0.0, 0.0, 4.0, 0.0, 0.0], ctm_scale_arr);

    let ctm_rot = CurTransMat::Rotate(30.0);
    let ctm_rot_arr: [f64; 6] = ctm_rot.into();
    assert_eq!([0.8660254037844384, 0.5000000000000004, -0.5000000000000004, 0.8660254037844384, 0.0, 0.0], ctm_rot_arr);
}
