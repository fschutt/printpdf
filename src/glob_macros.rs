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

macro_rules! add_operation {
    ($d: expr, $e: expr) => (let doc = $d.document.upgrade().unwrap();
		           let mut doc = doc.lock().unwrap();

		           doc.pages.get_mut($d.page.0).unwrap()
		               .layers.get_mut($d.layer.0).unwrap()
		                   .layer_stream.add_operation($e);)
}

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