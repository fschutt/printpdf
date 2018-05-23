//! Errors for printpdf

use std::error::Error as IError;
use std::io::Error as IoError;
use rusttype::Error as RusttypeError;
use std::fmt;

/// error_chain and failure are certainly nice, but completely overengineered
/// for this use-case. For example, neither of them allow error localization.
/// Additionally, debugging macros can get hairy really quick and matching with
/// `*e.kind()` or doing From conversions for other errors is really hard to do.
///
/// So in this case, the best form of error handling is to use the simple Rust-native
/// way: Just enums, `From` + pattern matching. No macros, except for this one.
///
/// What this macro does is (simplified): `impl From<$a> for $b { $b::$variant(error) }`
macro_rules! impl_from {
    ($from:ident, $to:ident::$variant:ident) => (
        impl From<$from> for $to {
            fn from(err: $from) -> Self {
                $to::$variant(err.into())
            }
        }
    )
}

#[derive(Debug)]
pub enum Error {
    /// External: std::io::Error
    Io(IoError),
    /// External: rusttype::Error
    Rusttype(RusttypeError),
    /// PDF error
    Pdf(PdfError),
    /// Indexing error (please report if this happens, shouldn't happen)
    Index(IndexError),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum PdfError {
    FontFaceError,
}

impl fmt::Display for PdfError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.description())
    }
}

impl IError for PdfError {
    fn description(&self) -> &str {
        use self::PdfError::*;
        match *self {
            FontFaceError => "Invalid or corrupt font face",
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum IndexError {
    PdfPageIndexError,
    PdfLayerIndexError,
    PdfMarkerIndexError,
}

impl fmt::Display for IndexError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.description())
    }
}

impl IError for IndexError {
    fn description(&self) -> &str {
        use self::IndexError::*;
        match *self {
            PdfPageIndexError => "Page index out of bounds",
            PdfLayerIndexError => "PDF layer index out of bounds",
            PdfMarkerIndexError => "PDF layer index out of bounds",
        }
    }
}

impl_from!(IoError, Error::Io);
impl_from!(RusttypeError, Error::Rusttype);
impl_from!(PdfError, Error::Pdf);
impl_from!(IndexError, Error::Index);



