use crate::glob_defines::{
    OP_PATH_CONST_3BEZIER_V1, OP_PATH_CONST_3BEZIER_V2, OP_PATH_CONST_4BEZIER,
    OP_PATH_CONST_LINE_TO, OP_PATH_CONST_MOVE_TO, OP_PATH_PAINT_END, OP_PATH_PAINT_STROKE,
    OP_PATH_PAINT_STROKE_CLOSE,
};
use crate::path::{PaintMode, WindingOrder};
use crate::Point;
use lopdf;
use std::iter::{FromIterator, IntoIterator};

#[derive(Debug, Clone, Default)]
pub struct Line {
    /// 2D Points for the line
    pub points: Vec<(Point, bool)>,
    /// Is the line closed or open?
    pub is_closed: bool,
}

impl FromIterator<(Point, bool)> for Line {
    fn from_iter<I: IntoIterator<Item = (Point, bool)>>(iter: I) -> Self {
        let mut points = Vec::new();
        for i in iter {
            points.push(i);
        }
        Line {
            points,
            ..Default::default()
        }
    }
}

impl Line {
    /// Sets if the line is closed or not
    #[inline]
    pub fn set_closed(&mut self, is_closed: bool) {
        self.is_closed = is_closed;
    }

    pub fn into_stream_op(self) -> Vec<lopdf::content::Operation> {
        use lopdf::content::Operation;
        let mut operations = Vec::<Operation>::new();

        if self.points.is_empty() {
            return operations;
        };

        operations.push(Operation::new(
            OP_PATH_CONST_MOVE_TO,
            vec![self.points[0].0.x.into(), self.points[0].0.y.into()],
        ));

        // Skip first element
        let mut current = 1;
        let max_len = self.points.len();

        // Loop over every points, determine if v, y, c or l operation should be used and build
        // curve / line accordingly
        while current < max_len {
            let p1 = &self.points[current - 1]; // prev pt
            let p2 = &self.points[current]; // current pt

            if p1.1 && p2.1 {
                // current point is a bezier handle
                // valid bezier curve must have two sequential bezier handles
                // we also can"t build a valid cubic bezier curve if the cuve contains less than
                // four points. If p3 or p4 is marked as "next point is bezier handle" or not, doesn"t matter
                if let Some(p3) = self.points.get(current + 1) {
                    if let Some(p4) = self.points.get(current + 2) {
                        if p1.0 == p2.0 {
                            // first control point coincides with initial point of curve
                            operations.push(Operation::new(
                                OP_PATH_CONST_3BEZIER_V1,
                                vec![p3.0.x.into(), p3.0.y.into(), p4.0.x.into(), p4.0.y.into()],
                            ));
                        } else if p2.0 == p3.0 {
                            // first control point coincides with final point of curve
                            operations.push(Operation::new(
                                OP_PATH_CONST_3BEZIER_V2,
                                vec![p2.0.x.into(), p2.0.y.into(), p4.0.x.into(), p4.0.y.into()],
                            ));
                        } else {
                            // regular bezier curve with four points
                            operations.push(Operation::new(
                                OP_PATH_CONST_4BEZIER,
                                vec![
                                    p2.0.x.into(),
                                    p2.0.y.into(),
                                    p3.0.x.into(),
                                    p3.0.y.into(),
                                    p4.0.x.into(),
                                    p4.0.y.into(),
                                ],
                            ));
                        }
                        current += 3;
                        continue;
                    }
                }
            }

            // normal straight line
            operations.push(Operation::new(
                OP_PATH_CONST_LINE_TO,
                vec![p2.0.x.into(), p2.0.y.into()],
            ));
            current += 1;
        }

        // not filled, not closed but only stroked (regular path)
        if self.is_closed {
            operations.push(Operation::new(OP_PATH_PAINT_STROKE_CLOSE, vec![]));
        } else {
            operations.push(Operation::new(OP_PATH_PAINT_STROKE, vec![]));
        }

        operations
    }
}

#[derive(Debug, Clone, Default)]
pub struct Polygon {
    /// 2D Points for the line
    pub rings: Vec<Vec<(Point, bool)>>,
    /// What type of polygon is this?
    pub mode: PaintMode,
    /// Winding order to use for constructing this polygon
    pub winding_order: WindingOrder,
}

impl FromIterator<(Point, bool)> for Polygon {
    fn from_iter<I: IntoIterator<Item = (Point, bool)>>(iter: I) -> Self {
        let mut points = Vec::new();
        for i in iter {
            points.push(i);
        }
        Polygon {
            rings: vec![points],
            ..Default::default()
        }
    }
}

impl Polygon {
    pub fn into_stream_op(self) -> Vec<lopdf::content::Operation> {
        use lopdf::content::Operation;
        let mut operations = Vec::<Operation>::new();

        if self.rings.is_empty() {
            return operations;
        };

        for ring in self.rings.iter() {
            operations.push(Operation::new(
                OP_PATH_CONST_MOVE_TO,
                vec![ring[0].0.x.into(), ring[0].0.y.into()],
            ));

            // Skip first element
            let mut current = 1;
            let max_len = ring.len();

            // Loop over every points, determine if v, y, c or l operation should be used and build
            // curve / line accordingly
            while current < max_len {
                let p1 = &ring[current - 1]; // prev pt
                let p2 = &ring[current]; // current pt

                if p1.1 && p2.1 {
                    // current point is a bezier handle
                    // valid bezier curve must have two sequential bezier handles
                    // we also can"t build a valid cubic bezier curve if the cuve contains less than
                    // four points. If p3 or p4 is marked as "next point is bezier handle" or not, doesn"t matter
                    if let Some(p3) = ring.get(current + 1) {
                        if let Some(p4) = ring.get(current + 2) {
                            if p1.0 == p2.0 {
                                // first control point coincides with initial point of curve
                                operations.push(Operation::new(
                                    OP_PATH_CONST_3BEZIER_V1,
                                    vec![
                                        p3.0.x.into(),
                                        p3.0.y.into(),
                                        p4.0.x.into(),
                                        p4.0.y.into(),
                                    ],
                                ));
                            } else if p2.0 == p3.0 {
                                // first control point coincides with final point of curve
                                operations.push(Operation::new(
                                    OP_PATH_CONST_3BEZIER_V2,
                                    vec![
                                        p2.0.x.into(),
                                        p2.0.y.into(),
                                        p4.0.x.into(),
                                        p4.0.y.into(),
                                    ],
                                ));
                            } else {
                                // regular bezier curve with four points
                                operations.push(Operation::new(
                                    OP_PATH_CONST_4BEZIER,
                                    vec![
                                        p2.0.x.into(),
                                        p2.0.y.into(),
                                        p3.0.x.into(),
                                        p3.0.y.into(),
                                        p4.0.x.into(),
                                        p4.0.y.into(),
                                    ],
                                ));
                            }
                            current += 3;
                            continue;
                        }
                    }
                }

                // normal straight line
                operations.push(Operation::new(
                    OP_PATH_CONST_LINE_TO,
                    vec![p2.0.x.into(), p2.0.y.into()],
                ));
                current += 1;
            }
        }

        match self.mode {
            PaintMode::Clip => {
                // set the path as a clipping path
                operations.push(Operation::new(self.winding_order.get_clip_op(), vec![]));
            }
            PaintMode::Fill => {
                // is not stroked, only filled
                // closed-ness doesn't matter in this case, an area is always closed
                operations.push(Operation::new(self.winding_order.get_fill_op(), vec![]));
            }
            PaintMode::Stroke => {
                // same as line with is_closed = true
                operations.push(Operation::new(OP_PATH_PAINT_STROKE_CLOSE, vec![]));
            }
            PaintMode::FillStroke => {
                operations.push(Operation::new(
                    self.winding_order.get_fill_stroke_close_op(),
                    vec![],
                ));
            }
        }

        if !operations.is_empty() {
            operations.push(Operation::new(OP_PATH_PAINT_END, vec![]));
        }

        operations
    }
}

