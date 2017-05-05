//! Macros for printpdf

/// Convert millimeter to points
#[macro_export]
macro_rules! mm_to_pt {
    ($mm: expr) => ($mm * 2.834646_f64);
}

/// Simple macro to cut down on typing when making a simple operation
macro_rules! operation {
    ($e: expr) => (vec![lopdf::content::Operation {
                      operator: $e.into(),
                      operands: vec![],
                  }])
}