//! Macros for printpdf

/// Convert millimeter to points
#[macro_export]
macro_rules! mm_to_pt {
    ($mm: expr) => ($mm * 2.834646_f64);
}
/*
/// Simple macro to cut down on typing when making a simple operation
macro_rules! operation {
    ($e: expr) => (vec![lopdf::content::Operation {
                      operator: $e.into(),
                      operands: vec![],
                  }])
}
*/

#[macro_export]
macro_rules! max {
    ($x:expr) => ( $x );
    ($x:expr, $($xs:expr),+) => {
        {
            use std::cmp::max;
            max($x, max!( $($xs),+ ))
        }
    };
}

#[macro_export]
macro_rules! min {
    ($x:expr) => ( $x );
    ($x:expr, $($xs:expr),+) => {
        {
            use std::cmp::min;
            min($x, min!( $($xs),+ ))
        }
    };
}