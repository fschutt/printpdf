//! Current transformation matrix, for transforming shapes (rotate, translate, scale)

use lopdf;
use lopdf::content::Operation;
use crate::Pt;

/// PDF "current transformation matrix". Once set, will operate on all following shapes,
/// until the `layer.restore_graphics_state()` is called. It is important to
/// call `layer.save_graphics_state()` earlier.
#[derive(Debug, Copy, Clone)]
pub enum CurTransMat {
    /// Translation matrix (in points from bottom left corner)
    /// X and Y can have different values
    Translate(Pt, Pt),
    /// Rotation matrix (clockwise, in degrees)
    Rotate(f64),
    /// Combined rotate + translate matrix
    TranslateRotate(Pt, Pt, f64),
    /// Scale matrix (1.0 = 100% scale, no change)
    /// X and Y can have different values
    Scale(f64, f64),
    /// Raw (PDF-internal) PDF matrix
    Raw([f64;6]),
    /// Identity matrix
    Identity,
}

impl CurTransMat {
    pub fn combine_matrix(a: [f64;6], b: [f64;6]) -> [f64;6] {

        let a = [
            [a[0], a[1], 0.0,  0.0],
            [a[2], a[3], 0.0,  0.0],
            [0.0,  0.0,  1.0,  0.0],
            [a[4], a[5], 0.0,  1.0],
        ];

        let b = [
            [b[0], b[1], 0.0,  0.0],
            [b[2], b[3], 0.0,  0.0],
            [0.0,  0.0,  1.0,  0.0],
            [b[4], b[5], 0.0,  1.0],
        ];

        let result = [

            [
            a[0][0].mul_add(b[0][0], a[0][1].mul_add(b[1][0], a[0][2].mul_add(b[2][0], a[0][3] * b[3][0]))),
            a[0][0].mul_add(b[0][1], a[0][1].mul_add(b[1][1], a[0][2].mul_add(b[2][1], a[0][3] * b[3][1]))),
            a[0][0].mul_add(b[0][2], a[0][1].mul_add(b[1][2], a[0][2].mul_add(b[2][2], a[0][3] * b[3][2]))),
            a[0][0].mul_add(b[0][3], a[0][1].mul_add(b[1][3], a[0][2].mul_add(b[2][3], a[0][3] * b[3][3]))),
            ],
            [
            a[1][0].mul_add(b[0][0], a[1][1].mul_add(b[1][0], a[1][2].mul_add(b[2][0], a[1][3] * b[3][0]))),
            a[1][0].mul_add(b[0][1], a[1][1].mul_add(b[1][1], a[1][2].mul_add(b[2][1], a[1][3] * b[3][1]))),
            a[1][0].mul_add(b[0][2], a[1][1].mul_add(b[1][2], a[1][2].mul_add(b[2][2], a[1][3] * b[3][2]))),
            a[1][0].mul_add(b[0][3], a[1][1].mul_add(b[1][3], a[1][2].mul_add(b[2][3], a[1][3] * b[3][3]))),
            ],

            [
            a[2][0].mul_add(b[0][0], a[2][1].mul_add(b[1][0], a[2][2].mul_add(b[2][0], a[2][3] * b[3][0]))),
            a[2][0].mul_add(b[0][1], a[2][1].mul_add(b[1][1], a[2][2].mul_add(b[2][1], a[2][3] * b[3][1]))),
            a[2][0].mul_add(b[0][2], a[2][1].mul_add(b[1][2], a[2][2].mul_add(b[2][2], a[2][3] * b[3][2]))),
            a[2][0].mul_add(b[0][3], a[2][1].mul_add(b[1][3], a[2][2].mul_add(b[2][3], a[2][3] * b[3][3]))),
            ],

            [
            a[3][0].mul_add(b[0][0], a[3][1].mul_add(b[1][0], a[3][2].mul_add(b[2][0], a[3][3] * b[3][0]))),
            a[3][0].mul_add(b[0][1], a[3][1].mul_add(b[1][1], a[3][2].mul_add(b[2][1], a[3][3] * b[3][1]))),
            a[3][0].mul_add(b[0][2], a[3][1].mul_add(b[1][2], a[3][2].mul_add(b[2][2], a[3][3] * b[3][2]))),
            a[3][0].mul_add(b[0][3], a[3][1].mul_add(b[1][3], a[3][2].mul_add(b[2][3], a[3][3] * b[3][3]))),
            ],
        ];

        [result[0][0], result[0][1], result[1][0], result[1][1], result[3][0], result[3][1]]
    }
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
    Translate(Pt, Pt),
    /// Combined translate + rotate matrix
    TranslateRotate(Pt, Pt, f64),
    /// Raw matrix (/tm operator)
    Raw([f64;6]),
}

impl Into<[f64; 6]> for TextMatrix {
    fn into(self)
    -> [f64; 6]
    {
        use TextMatrix::*;
        match self {
            Translate(x, y) => { 
                // 1 0 0 1 x y cm 
                [ 1.0, 0.0, 0.0, 1.0, x.0, y.0 ]
            }
            Rotate(rot) => {
                let rad = (360.0 - rot).to_radians();
                [rad.cos(), -rad.sin(), rad.sin(), rad.cos(), 0.0, 0.0 ] /* cos sin -sin cos 0 0 cm */
            },
            Raw(r) => r.clone(),
            TranslateRotate(x, y, rot) => {
                let rad = (360.0 - rot).to_radians();
                [rad.cos(), -rad.sin(), rad.sin(), rad.cos(), x.0, y.0 ] /* cos sin -sin cos x y cm */
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
                [ 1.0, 0.0, 0.0, 1.0, x.0, y.0 ]
            }
            TranslateRotate(x, y, rot) => {
                let rad = (360.0 - rot).to_radians();
                [rad.cos(), -rad.sin(), rad.sin(), rad.cos(), x.0, y.0 ] /* cos sin -sin cos x y cm */
            }
            Rotate(rot) => { 
                // cos sin -sin cos 0 0 cm 
                let rad = (360.0 - rot).to_radians(); 
                [rad.cos(), -rad.sin(), rad.sin(), rad.cos(), 0.0, 0.0 ] 
            },
            Raw(r) => r.clone(),
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
    let ctm_trans = CurTransMat::Translate(Pt(150.0), Pt(50.0));
    let ctm_trans_arr: [f64; 6] = ctm_trans.into();
    assert_eq!([1.0_f64, 0.0, 0.0, 1.0, 150.0, 50.0], ctm_trans_arr);

    let ctm_scale = CurTransMat::Scale(2.0, 4.0);
    let ctm_scale_arr: [f64; 6] = ctm_scale.into();
    assert_eq!([2.0_f64, 0.0, 0.0, 4.0, 0.0, 0.0], ctm_scale_arr);

    let ctm_rot = CurTransMat::Rotate(30.0);
    let ctm_rot_arr: [f64; 6] = ctm_rot.into();
    assert_eq!([0.8660254037844384, 0.5000000000000004, -0.5000000000000004, 0.8660254037844384, 0.0, 0.0], ctm_rot_arr);
}
