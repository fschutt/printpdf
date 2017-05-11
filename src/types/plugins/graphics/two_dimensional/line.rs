use *;
use traits::*;
use types::indices::*;

#[derive(Debug, Clone)]
pub struct Line { 
    /// 2D Points for the line
    pub points: Vec<(Point, bool)>,
    /// Is the line closed or open?
    pub closed: bool,
}

// the current outline of the line
static mut CURRENT_OUTLINE: Outline = Outline {
                                        color: Color::Rgb(Rgb { r: 0.0, g: 0.0, b: 0.0, icc_profile: None }),
                                        thickness: 5,
                                      };

impl Line {



    /// Creates a new line from the given points
    /// Each point has a bool, indicating if the next point is a bezier curve
    /// This allows compression inside the pdf since PDF knows several operators for this.
    #[inline]
    pub fn new(points: Vec<(Point, bool)>, closed: bool)
    -> Self
    {
        Self {
            points,
            closed,
        }
    }

    /// Changes the outline for following lines
    pub fn set_outline(outline: Outline) 
    { 
        unsafe { CURRENT_OUTLINE = outline };
    }
}

impl IntoPdfStreamOperation for Line {

    fn into_stream_op(self: Box<Self>)
    -> Vec<lopdf::content::Operation>
    {
        /* &mut self, line: &Line, outline_col: Option<&Cmyk>, outline_pt: Option<i64>, fill_col: Option<&Cmyk> */
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

        match self.closed {
            true  => { operations.place_back() <- Operation::new(OP_PATH_PAINT_STROKE_CLOSE, vec![]); },
            false => { operations.place_back() <- Operation::new(OP_PATH_PAINT_STROKE, vec![]); },
        }        
        
        operations
    }

}