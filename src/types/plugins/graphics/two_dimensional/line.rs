use *;

#[derive(Debug, Clone)]
pub struct Line { 
    pub points: Vec<(Point, bool)>,
    pub style: Style,
}

impl Line {
    /// Creates a new line from the given points
    /// Each point has a bool, indicating if the next point is a bezier curve
    pub fn new() { }
}