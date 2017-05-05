use *;

#[derive(Debug, Clone)]
pub struct Line { 
    points: Vec<(Point, bool)>,
}