//! Macros for printpdf

/// Convert millimeter to points
#[macro_export]
macro_rules! mm_to_pt {
    ($mm: expr) => ($mm * 2.834_646_f64);
}
