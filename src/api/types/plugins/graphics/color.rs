use super::*;

#[derive(Debug, Clone)]
pub enum Color {
    Rbg(Rgb),
    Cmyk(Cmyk),
}

#[derive(Debug, Clone)]
pub struct Rgb {
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub color_space: Option<IccProfile>
}

impl Rgb {

    pub fn new(r: f64, g: f64, b: f64, color_space: Option<IccProfile>)
    -> Self
    {
        Self { r, g, b, color_space }
    }
}

#[derive(Debug, Clone)]
pub struct Cmyk {
    pub c: f64,
    pub m: f64,
    pub y: f64,
    pub k: f64,
    pub color_space: Option<IccProfile>
}

impl Cmyk {

    pub fn new(c: f64, m: f64, y: f64, k: f64, color_space: Option<IccProfile>)
    -> Self
    {
        Self { c, m, y, k, color_space }
    }
}