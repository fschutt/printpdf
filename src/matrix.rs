use serde_derive::{Deserialize, Serialize};

use crate::units::Pt;

/// PDF "current transformation matrix" (CTM) for the graphics state.
///
/// This transformation affects drawing operations and uses the PDF "cm" operator,
/// which is cumulative/additive to the graphics state stack. Each transformation
/// combines with the existing state rather than replacing it completely.
///
/// Once set, it operates on all following shapes until `restore_graphics_state()` is called.
/// It is important to call `save_graphics_state()` before applying transformations.
#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "type", content = "data")]
pub enum CurTransMat {
    /// Translation matrix (in points from bottom left corner)
    /// X and Y can have different values
    Translate(Pt, Pt),
    /// Rotation matrix (clockwise, in degrees)
    ///
    /// Note: This rotates around ORIGIN (0,0) of the CURRENT graphics state
    Rotate(f32),
    /// Combined rotate + translate matrix
    ///
    /// Rotates around the specified point, in ADDITION to the EXISTING
    /// matrix of the current graphics state
    TranslateRotate(Pt, Pt, f32),
    /// Scale matrix (1.0 = 100% scale, no change)
    /// X and Y can have different values
    Scale(f32, f32),
    /// Raw (PDF-internal) PDF matrix
    Raw([f32; 6]),
    /// Identity matrix
    Identity,
}

impl PartialEq for CurTransMat {
    fn eq(&self, other: &Self) -> bool {
        self.as_array() == other.as_array()
    }
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

/// Text matrix for text positioning and transformation in PDF.
///
/// IMPORTANT: Unlike CurTransMat, TextMatrix uses the PDF "Tm" operator which
/// **COMPLETELY REPLACES** the current text matrix rather than combining with it.
/// This means that setting a TextMatrix will reset any previously established
/// text position and transformation.
///
/// However, the text matrix is added ON TOP OF the "CurTransMat", it does not replace
/// the "CurTransMat", but it will replace previous operations such as "SetTextPosition".
///
/// This is why there is no simple "Rotate" variant - it would reset the text position
/// to the origin (0,0). Use TranslateRotate instead to maintain position.
///
/// Note: `TextScale` does not exist. Use `set_word_spacing()` and `set_character_spacing()`
/// to specify the scaling between words and characters.
#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TextMatrix {
    /// Text translate matrix: REPLACES the existing text cursor!
    /// This is equivalent to `SetTextCursor { x, y }`, since it will replace
    /// the position of the cursor. The x / y is relative to the pages lower left corner.
    Translate(Pt, Pt),
    /// Combined translate + rotate matrix: Rotates text around the specified point
    /// RELATIVE to the PAGE origin.
    ///
    /// Since the `Tm` operator replaces the matrix completely, you must specify
    /// both position (relative to the PAGE, not the current text) and rotation in one operation
    TranslateRotate(Pt, Pt, f32),
    /// Raw matrix for the PDF "Tm" operator
    Raw([f32; 6]),
}

impl PartialEq for TextMatrix {
    fn eq(&self, other: &Self) -> bool {
        self.as_array() == other.as_array()
    }
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
            Raw(r) => *r,
            TranslateRotate(x, y, rot) => {
                let rad = (360.0 - rot).to_radians();
                [rad.cos(), -rad.sin(), rad.sin(), rad.cos(), x.0, y.0] /* cos sin -sin cos x y
                                                                         * cm */
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
                [rad.cos(), -rad.sin(), rad.sin(), rad.cos(), x.0, y.0] /* cos sin -sin cos x y
                                                                         * cm */
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
