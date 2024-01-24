//! Current transformation matrix, for transforming shapes (rotate, translate, scale)

use crate::Pt;
use lopdf;
use lopdf::content::Operation;

/// PDF "current transformation matrix". Once set, will operate on all following shapes,
/// until the `layer.restore_graphics_state()` is called. It is important to
/// call `layer.save_graphics_state()` earlier.
#[derive(Debug, Copy, Clone)]
pub enum CurTransMat {
    /// Translation matrix (in points from bottom left corner)
    /// X and Y can have different values
    Translate(Pt, Pt),
    /// Rotation matrix (clockwise, in degrees)
    Rotate(f32),
    /// Combined rotate + translate matrix
    TranslateRotate(Pt, Pt, f32),
    /// Scale matrix (1.0 = 100% scale, no change)
    /// X and Y can have different values
    Scale(f32, f32),
    /// Raw (PDF-internal) PDF matrix
    Raw([f32; 6]),
    /// Identity matrix
    Identity,
}

impl CurTransMat {
    pub fn combine_matrix(a: [f32; 6], b: [f32; 6]) -> [f32; 6] {
        let a = [
            [a[0], a[1], 0.0, 0.0],
            [a[2], a[3], 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [a[4], a[5], 0.0, 1.0],
        ];

        let b = [
            [b[0], b[1], 0.0, 0.0],
            [b[2], b[3], 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [b[4], b[5], 0.0, 1.0],
        ];

        let result = [
            [
                a[0][0].mul_add(
                    b[0][0],
                    a[0][1].mul_add(b[1][0], a[0][2].mul_add(b[2][0], a[0][3] * b[3][0])),
                ),
                a[0][0].mul_add(
                    b[0][1],
                    a[0][1].mul_add(b[1][1], a[0][2].mul_add(b[2][1], a[0][3] * b[3][1])),
                ),
                a[0][0].mul_add(
                    b[0][2],
                    a[0][1].mul_add(b[1][2], a[0][2].mul_add(b[2][2], a[0][3] * b[3][2])),
                ),
                a[0][0].mul_add(
                    b[0][3],
                    a[0][1].mul_add(b[1][3], a[0][2].mul_add(b[2][3], a[0][3] * b[3][3])),
                ),
            ],
            [
                a[1][0].mul_add(
                    b[0][0],
                    a[1][1].mul_add(b[1][0], a[1][2].mul_add(b[2][0], a[1][3] * b[3][0])),
                ),
                a[1][0].mul_add(
                    b[0][1],
                    a[1][1].mul_add(b[1][1], a[1][2].mul_add(b[2][1], a[1][3] * b[3][1])),
                ),
                a[1][0].mul_add(
                    b[0][2],
                    a[1][1].mul_add(b[1][2], a[1][2].mul_add(b[2][2], a[1][3] * b[3][2])),
                ),
                a[1][0].mul_add(
                    b[0][3],
                    a[1][1].mul_add(b[1][3], a[1][2].mul_add(b[2][3], a[1][3] * b[3][3])),
                ),
            ],
            [
                a[2][0].mul_add(
                    b[0][0],
                    a[2][1].mul_add(b[1][0], a[2][2].mul_add(b[2][0], a[2][3] * b[3][0])),
                ),
                a[2][0].mul_add(
                    b[0][1],
                    a[2][1].mul_add(b[1][1], a[2][2].mul_add(b[2][1], a[2][3] * b[3][1])),
                ),
                a[2][0].mul_add(
                    b[0][2],
                    a[2][1].mul_add(b[1][2], a[2][2].mul_add(b[2][2], a[2][3] * b[3][2])),
                ),
                a[2][0].mul_add(
                    b[0][3],
                    a[2][1].mul_add(b[1][3], a[2][2].mul_add(b[2][3], a[2][3] * b[3][3])),
                ),
            ],
            [
                a[3][0].mul_add(
                    b[0][0],
                    a[3][1].mul_add(b[1][0], a[3][2].mul_add(b[2][0], a[3][3] * b[3][0])),
                ),
                a[3][0].mul_add(
                    b[0][1],
                    a[3][1].mul_add(b[1][1], a[3][2].mul_add(b[2][1], a[3][3] * b[3][1])),
                ),
                a[3][0].mul_add(
                    b[0][2],
                    a[3][1].mul_add(b[1][2], a[3][2].mul_add(b[2][2], a[3][3] * b[3][2])),
                ),
                a[3][0].mul_add(
                    b[0][3],
                    a[3][1].mul_add(b[1][3], a[3][2].mul_add(b[2][3], a[3][3] * b[3][3])),
                ),
            ],
        ];

        [
            result[0][0],
            result[0][1],
            result[1][0],
            result[1][1],
            result[3][0],
            result[3][1],
        ]
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
    Rotate(f32),
    /// Text translate matrix, used for indenting (transforming) text
    /// (different to regular text placement)
    Translate(Pt, Pt),
    /// Combined translate + rotate matrix
    TranslateRotate(Pt, Pt, f32),
    /// Raw matrix (/tm operator)
    Raw([f32; 6]),
}

impl From<TextMatrix> for [f32; 6] {
    fn from(val: TextMatrix) -> Self {
        use crate::TextMatrix::*;
        match val {
            Translate(x, y) => {
                // 1 0 0 1 x y cm
                [1.0, 0.0, 0.0, 1.0, x.0, y.0]
            }
            Rotate(rot) => {
                let rad = (360.0 - rot).to_radians();
                [rad.cos(), -rad.sin(), rad.sin(), rad.cos(), 0.0, 0.0] /* cos sin -sin cos 0 0 cm */
            }
            Raw(r) => r,
            TranslateRotate(x, y, rot) => {
                let rad = (360.0 - rot).to_radians();
                [rad.cos(), -rad.sin(), rad.sin(), rad.cos(), x.0, y.0] /* cos sin -sin cos x y cm */
            }
        }
    }
}

impl From<CurTransMat> for [f32; 6] {
    fn from(val: CurTransMat) -> Self {
        use crate::CurTransMat::*;
        match val {
            Translate(x, y) => {
                // 1 0 0 1 x y cm
                [1.0, 0.0, 0.0, 1.0, x.0, y.0]
            }
            TranslateRotate(x, y, rot) => {
                let rad = (360.0 - rot).to_radians();
                [rad.cos(), -rad.sin(), rad.sin(), rad.cos(), x.0, y.0] /* cos sin -sin cos x y cm */
            }
            Rotate(rot) => {
                // cos sin -sin cos 0 0 cm
                let rad = (360.0 - rot).to_radians();
                [rad.cos(), -rad.sin(), rad.sin(), rad.cos(), 0.0, 0.0]
            }
            Raw(r) => r,
            Scale(x, y) => {
                // x 0 0 y 0 0 cm
                [x, 0.0, 0.0, y, 0.0, 0.0]
            }
            Identity => [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
        }
    }
}

impl From<CurTransMat> for Operation {
    fn from(val: CurTransMat) -> Self {
        use lopdf::Object::*;
        let matrix_nums: [f32; 6] = val.into();
        let matrix: Vec<lopdf::Object> = matrix_nums.iter().copied().map(Real).collect();
        Operation::new("cm", matrix)
    }
}

impl From<TextMatrix> for Operation {
    fn from(val: TextMatrix) -> Self {
        use lopdf::Object::*;
        let matrix_nums: [f32; 6] = val.into();
        let matrix: Vec<lopdf::Object> = matrix_nums.iter().copied().map(Real).collect();
        Operation::new("Tm", matrix)
    }
}

impl From<CurTransMat> for lopdf::Object {
    fn from(val: CurTransMat) -> Self {
        use lopdf::Object::*;
        let matrix_nums: [f32; 6] = val.into();
        Array(matrix_nums.iter().copied().map(Real).collect())
    }
}

#[test]
fn test_ctm_translate() {
    use self::*;

    // test that the translation matrix look like what PDF expects
    let ctm_trans = CurTransMat::Translate(Pt(150.0), Pt(50.0));
    let ctm_trans_arr: [f32; 6] = ctm_trans.into();
    assert_eq!([1.0_f32, 0.0, 0.0, 1.0, 150.0, 50.0], ctm_trans_arr);

    let ctm_scale = CurTransMat::Scale(2.0, 4.0);
    let ctm_scale_arr: [f32; 6] = ctm_scale.into();
    assert_eq!([2.0_f32, 0.0, 0.0, 4.0, 0.0, 0.0], ctm_scale_arr);

    let ctm_rot = CurTransMat::Rotate(30.0);
    let ctm_rot_arr: [f32; 6] = ctm_rot.into();
    assert_eq!([0.8660253, 0.5000002, -0.5000002, 0.8660253, 0.0, 0.0], ctm_rot_arr);
}
