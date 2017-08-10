#![cfg_attr(feature = "cargo-clippy", allow(module_inception))]

//! Errors for printpdf

pub mod errors;
pub mod pdf_error;
pub mod index_error;

pub use self::errors::*;
pub use self::pdf_error::ErrorKind::*;
pub use self::index_error::*;
pub use self::errors::ErrorKind::*;

