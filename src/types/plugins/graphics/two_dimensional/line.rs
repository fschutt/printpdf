use lopdf;
use glob_defines::{
    OP_PATH_CONST_MOVE_TO, OP_PATH_CONST_3BEZIER_V1, OP_PATH_CONST_3BEZIER_V2, OP_PATH_CONST_4BEZIER,
    OP_PATH_CONST_LINE_TO, OP_PATH_PAINT_FILL_STROKE_CLOSE_NZ, OP_PATH_PAINT_FILL_NZ,
    OP_PATH_PAINT_STROKE_CLOSE, OP_PATH_PAINT_STROKE, OP_PATH_PAINT_END,
};
use Point;
use std::iter::{FromIterator, IntoIterator};

#[derive(Debug, Clone)]
pub struct Line {
    /// 2D Points for the line
    pub points: Vec<(Point, bool)>,
    /// Is the line closed or open?
    pub is_closed: bool,
    /// Should the line be filled (via winding-number rule), for polygons
    pub has_fill: bool,
    /// Should the line have an outline (stroke)?
    pub has_stroke: bool,
    /// Is this line a clipping path?
    pub is_clipping_path: bool,
}

impl Default for Line {
    fn default() -> Self {
        Self {
            points: Vec::new(),
            is_closed: false,
            has_fill: false,
            has_stroke: false,
            is_clipping_path: false,
        }
    }
}

impl FromIterator<(Point, bool)> for Line {
    fn from_iter<I: IntoIterator<Item=(Point, bool)>>(iter: I) -> Self {
        let mut points = Vec::new();
        for i in iter {
            points.push(i);
        }
        Line {
            points: points,
            .. Default::default()
        }
    }
}

impl Line {

    /// Sets if the line is closed or not
    #[inline]
    pub fn set_closed(&mut self, is_closed: bool) {
        self.is_closed = is_closed;
    }

    /// Sets if the line is filled
    #[inline]
    pub fn set_fill(&mut self, has_fill: bool) {
        self.has_fill = has_fill;
    }

    /// Sets if the line is stroked (has an outline)
    #[inline]
    pub fn set_stroke(&mut self, has_stroke: bool) {
        self.has_stroke = has_stroke;
    }

    /// Sets if the line is a clipping path
    #[inline]
    pub fn set_as_clipping_path(&mut self, is_clipping_path: bool) {
        self.is_clipping_path = is_clipping_path;
    }

    pub fn into_stream_op(self)
    -> Vec<lopdf::content::Operation>
    {
        use lopdf::content::Operation;
        let mut operations = Vec::<Operation>::new();

        if self.points.is_empty() { return operations; };

        operations.push(Operation::new(OP_PATH_CONST_MOVE_TO, vec![self.points[0].0.x.into(), self.points[0].0.y.into()]));

        // Skip first element
        let mut current = 1;
        let max_len = self.points.len();

        // Loop over every points, determine if v, y, c or l operation should be used and build
        // curve / line accordingly
        while current < max_len {
            let p1 = &self.points[current - 1];                      // prev pt
            let p2 = &self.points[current];                          // current pt


            if p1.1 && p2.1 {
                // current point is a bezier handle
                // valid bezier curve must have two sequential bezier handles
                // we also can"t build a valid cubic bezier curve if the cuve contains less than
                // four points. If p3 or p4 is marked as "next point is bezier handle" or not, doesn"t matter
                if let Some(p3) = self.points.get(current + 1){
                    if let Some(p4) = self.points.get(current + 2){
                        if p1.0 == p2.0 {
                            // first control point coincides with initial point of curve
                            operations.push(Operation::new(OP_PATH_CONST_3BEZIER_V1, vec![p3.0.x.into(), p3.0.y.into(), p4.0.x.into(), p4.0.y.into()]));
                        }else if p2.0 == p3.0 {
                            // first control point coincides with final point of curve
                            operations.push(Operation::new(OP_PATH_CONST_3BEZIER_V2, vec![p2.0.x.into(), p2.0.y.into(), p4.0.x.into(), p4.0.y.into()]));
                        }else{
                            // regular bezier curve with four points
                            operations.push(Operation::new(OP_PATH_CONST_4BEZIER, vec![p2.0.x.into(), p2.0.y.into(), p3.0.x.into(), p3.0.y.into(), p4.0.x.into(), p4.0.y.into()]));
                        }
                        current += 3;
                        continue;
                    }
                }
            }

            // normal straight line
            operations.push(Operation::new(OP_PATH_CONST_LINE_TO, vec![p2.0.x.into(), p2.0.y.into()]));
            current += 1;
        }

        // how to paint the path
        if self.has_stroke {
            if self.has_fill {
                if self.is_closed {
                    // is filled and stroked and closed
                    operations.push(Operation::new(OP_PATH_PAINT_FILL_STROKE_CLOSE_NZ, vec![]));
                } else {
                    // is filled and stroked but not closed
                    operations.push(Operation::new(OP_PATH_PAINT_FILL_NZ, vec![]));
                }
            } else if self.is_closed {
                // not filled, but stroked and closed
                operations.push(Operation::new(OP_PATH_PAINT_STROKE_CLOSE, vec![]));
            } else {
                // not filled, not closed but only stroked (regular path)
                operations.push(Operation::new(OP_PATH_PAINT_STROKE, vec![]));
            }
        } else if self.has_fill {
            // is not stroked, only filled
            // closed-ness doesn't matter in this case, an area is always closed
            operations.push(Operation::new(OP_PATH_PAINT_FILL_NZ, vec![]));
        } else {
            // no painting operation nothing, path is invisible, only end the path
            operations.push(Operation::new(OP_PATH_PAINT_END, vec![]));
        }

        operations
    }
}
