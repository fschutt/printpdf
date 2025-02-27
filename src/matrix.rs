//! Current transformation matrix, for transforming shapes (rotate, translate, scale)

use serde_derive::{Deserialize, Serialize};

use crate::units::Pt;

/// PDF "current transformation matrix". Once set, will operate on all following shapes,
/// until the `layer.restore_graphics_state()` is called. It is important to
/// call `layer.save_graphics_state()` earlier.
#[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "type", content = "data")]
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
    pub fn as_css_val(&self) -> String {
        let m = self.as_array();
        format!(
            "matrix({} {} {} {} {} {})",
            m[0], m[1], m[2], m[3], m[4], m[5]
        )
    }

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
                mul_add(
                    a[0][0],
                    b[0][0],
                    mul_add(
                        a[0][1],
                        b[1][0],
                        mul_add(a[0][2], b[2][0], a[0][3] * b[3][0]),
                    ),
                ),
                mul_add(
                    a[0][0],
                    b[0][1],
                    mul_add(
                        a[0][1],
                        b[1][1],
                        mul_add(a[0][2], b[2][1], a[0][3] * b[3][1]),
                    ),
                ),
                mul_add(
                    a[0][0],
                    b[0][2],
                    mul_add(
                        a[0][1],
                        b[1][2],
                        mul_add(a[0][2], b[2][2], a[0][3] * b[3][2]),
                    ),
                ),
                mul_add(
                    a[0][0],
                    b[0][3],
                    mul_add(
                        a[0][1],
                        b[1][3],
                        mul_add(a[0][2], b[2][3], a[0][3] * b[3][3]),
                    ),
                ),
            ],
            [
                mul_add(
                    a[1][0],
                    b[0][0],
                    mul_add(
                        a[1][1],
                        b[1][0],
                        mul_add(a[1][2], b[2][0], a[1][3] * b[3][0]),
                    ),
                ),
                mul_add(
                    a[1][0],
                    b[0][1],
                    mul_add(
                        a[1][1],
                        b[1][1],
                        mul_add(a[1][2], b[2][1], a[1][3] * b[3][1]),
                    ),
                ),
                mul_add(
                    a[1][0],
                    b[0][2],
                    mul_add(
                        a[1][1],
                        b[1][2],
                        mul_add(a[1][2], b[2][2], a[1][3] * b[3][2]),
                    ),
                ),
                mul_add(
                    a[1][0],
                    b[0][3],
                    mul_add(
                        a[1][1],
                        b[1][3],
                        mul_add(a[1][2], b[2][3], a[1][3] * b[3][3]),
                    ),
                ),
            ],
            [
                mul_add(
                    a[2][0],
                    b[0][0],
                    mul_add(
                        a[2][1],
                        b[1][0],
                        mul_add(a[2][2], b[2][0], a[2][3] * b[3][0]),
                    ),
                ),
                mul_add(
                    a[2][0],
                    b[0][1],
                    mul_add(
                        a[2][1],
                        b[1][1],
                        mul_add(a[2][2], b[2][1], a[2][3] * b[3][1]),
                    ),
                ),
                mul_add(
                    a[2][0],
                    b[0][2],
                    mul_add(
                        a[2][1],
                        b[1][2],
                        mul_add(a[2][2], b[2][2], a[2][3] * b[3][2]),
                    ),
                ),
                mul_add(
                    a[2][0],
                    b[0][3],
                    mul_add(
                        a[2][1],
                        b[1][3],
                        mul_add(a[2][2], b[2][3], a[2][3] * b[3][3]),
                    ),
                ),
            ],
            [
                mul_add(
                    a[3][0],
                    b[0][0],
                    mul_add(
                        a[3][1],
                        b[1][0],
                        mul_add(a[3][2], b[2][0], a[3][3] * b[3][0]),
                    ),
                ),
                mul_add(
                    a[3][0],
                    b[0][1],
                    mul_add(
                        a[3][1],
                        b[1][1],
                        mul_add(a[3][2], b[2][1], a[3][3] * b[3][1]),
                    ),
                ),
                mul_add(
                    a[3][0],
                    b[0][2],
                    mul_add(
                        a[3][1],
                        b[1][2],
                        mul_add(a[3][2], b[2][2], a[3][3] * b[3][2]),
                    ),
                ),
                mul_add(
                    a[3][0],
                    b[0][3],
                    mul_add(
                        a[3][1],
                        b[1][3],
                        mul_add(a[3][2], b[2][3], a[3][3] * b[3][3]),
                    ),
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

/// Multiply add. Computes `(self * a) + b` with workaround for
/// arm-unknown-linux-gnueabi.
///
/// `{f32, f64}::mul_add` is completly broken on arm-unknown-linux-gnueabi.
/// See issue https://github.com/rust-lang/rust/issues/46950.
#[inline(always)]
fn mul_add(a: f32, b: f32, c: f32) -> f32 {
    if cfg!(all(
        target_arch = "arm",
        target_os = "linux",
        target_env = "gnu"
    )) {
        // Workaround has two rounding errors and less accurate result,
        // but for PDF it doesn't matter much.
        (a * b) + c
    } else {
        a.mul_add(b, c)
    }
}

/// Text matrix. Text placement is a bit different, but uses the same
/// concepts as a CTM that's why it's merged here
///
/// Note: `TextScale` does not exist. Use `layer.set_word_spacing()`
/// and `layer.set_character_spacing()` to specify the scaling between words
/// and characters.
#[derive(Debug, Copy, PartialEq, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
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

impl TextMatrix {
    pub fn as_css_val(&self, invert_y: bool) -> String {
        let m = self.as_array();
        let factor = if invert_y { -1.0 } else { 1.0 };
        format!(
            "matrix({} {} {} {} {} {})",
            m[0],
            m[1],
            m[2],
            m[3],
            m[4],
            m[5] * factor
        )
    }

    pub fn as_array(&self) -> [f32; 6] {
        use self::TextMatrix::*;
        match self {
            Translate(x, y) => {
                // 1 0 0 1 x y cm
                [1.0, 0.0, 0.0, 1.0, x.0, y.0]
            }
            Rotate(rot) => {
                let rad = (360.0 - rot).to_radians();
                [rad.cos(), -rad.sin(), rad.sin(), rad.cos(), 0.0, 0.0] /* cos sin -sin cos 0 0 cm */
            }
            Raw(r) => *r,
            TranslateRotate(x, y, rot) => {
                let rad = (360.0 - rot).to_radians();
                [rad.cos(), -rad.sin(), rad.sin(), rad.cos(), x.0, y.0] /* cos sin -sin cos x y cm */
            }
        }
    }
}

impl CurTransMat {
    pub fn as_array(&self) -> [f32; 6] {
        use self::CurTransMat::*;
        match self {
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
            Raw(r) => *r,
            Scale(x, y) => {
                // x 0 0 y 0 0 cm
                [*x, 0.0, 0.0, *y, 0.0, 0.0]
            }
            Identity => [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
        }
    }
}

#[test]
fn test_ctm_translate() {
    use self::*;

    // test that the translation matrix look like what PDF expects
    let ctm_trans = CurTransMat::Translate(Pt(150.0), Pt(50.0));
    let ctm_trans_arr: [f32; 6] = ctm_trans.as_array();
    assert_eq!([1.0_f32, 0.0, 0.0, 1.0, 150.0, 50.0], ctm_trans_arr);

    let ctm_scale = CurTransMat::Scale(2.0, 4.0);
    let ctm_scale_arr: [f32; 6] = ctm_scale.as_array();
    assert_eq!([2.0_f32, 0.0, 0.0, 4.0, 0.0, 0.0], ctm_scale_arr);

    let ctm_rot = CurTransMat::Rotate(30.0);
    let ctm_rot_arr: [f32; 6] = ctm_rot.as_array();
    assert_eq!(
        [0.8660253, 0.5000002, -0.5000002, 0.8660253, 0.0, 0.0],
        ctm_rot_arr
    );
}
