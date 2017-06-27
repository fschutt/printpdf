use *;
use traits::*;

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
}

impl Line {

    /// Creates a new line from the given points
    /// Each point has a bool, indicating if the next point is a bezier curve
    /// This allows compression inside the pdf since PDF knows several operators for this.
    #[inline]
    pub fn new(points: Vec<(Point, bool)>, 
               is_closed: bool, 
               has_fill: bool,
               has_stroke: bool)
    -> Self
    {
        Self {
            points,
            is_closed,
            has_fill,
            has_stroke,
        }
    }

    /// Changes the fill color for following lines
    #[inline]
    pub fn set_fill(fill: Fill) 
    { 
        *super::CURRENT_FILL.lock().unwrap() = fill;
    }

    /// Changes the outline for following lines
    #[inline]
    pub fn set_outline(outline: Outline) 
    { 
        *super::CURRENT_OUTLINE.lock().unwrap() = outline;
    }
}

impl IntoPdfStreamOperation for Line {

    fn into_stream_op(self: Box<Self>)
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
                    operations.place_back() <- Operation::new(OP_PATH_PAINT_FILL_STROKE_CLOSE_NZ, vec![]);
                } else {
                    // is filled and stroked but not closed
                    operations.place_back() <- Operation::new(OP_PATH_PAINT_FILL_NZ, vec![]);
                }
            } else {
                if self.is_closed {
                    // not filled, but stroked and closed
                    operations.place_back() <- Operation::new(OP_PATH_PAINT_STROKE_CLOSE, vec![]);
                } else {
                    // not filled, not closed but only stroked (regular path)
                    operations.place_back() <- Operation::new(OP_PATH_PAINT_STROKE, vec![]);
                } 
            } 
        } else {
            if self.has_fill {
                // is not stroked, only filled
                // closed-ness doesn't matter in this case, an area is always closed
                operations.place_back() <- Operation::new(OP_PATH_PAINT_FILL_NZ, vec![]);
            } else {
                // no painting operation nothing, path is invisible, only end the path
                operations.place_back() <- Operation::new(OP_PATH_PAINT_END, vec![]);
            }
        }

        operations
    }

}