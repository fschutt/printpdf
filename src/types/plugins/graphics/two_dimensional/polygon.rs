
use *;

/// Polyogn (similar to line, but can have a fill color)
#[derive(Debug, Clone)]
pub struct Polygon { 
    /// Points of the polygon
    pub points: Vec<(Point, bool)>,
}

static mut CURRENT_OUTLINE: Outline = Outline {
                                        color: Color::Rgb(Rgb { r: 0.0, g: 0.0, b: 0.0, icc_profile: None }),
                                        thickness: 5,
                                      };

static mut CURRENT_FILL: Fill = Fill {
                                    color: Color::Rgb(Rgb { r: 0.0, g: 0.0, b: 0.0, icc_profile: None }),
                                };

impl Polygon {

    /// Creates a new line from the given points
    /// Each point has a bool, indicating if the next point is a bezier curve
    /// This allows compression inside the pdf
    #[inline]
    pub fn new(points: Vec<(Point, bool)>)
    -> Self
    {
        Self {
            points
        }
    }

    /// Changes the outline for following lines
    #[inline]
    pub fn set_outline(outline: Outline) 
    { 
        unsafe { CURRENT_OUTLINE = outline };
    }

    /// Changes the outline for following lines
    #[inline]
    pub fn set_fill(fill: Fill) 
    { 
        unsafe { CURRENT_FILL = fill };
    }
}
