extern crate error_chain;
extern crate image;
extern crate freetype as ft;

use super::*;

error_chain! {

    types {
        Error, ErrorKind, ResultExt, Result;
    }

    foreign_links {
        IoError(::std::io::Error);
        FontError(ft::Error);
    }

    links {
        PDFError(pdf_error::Error, pdf_error::ErrorKind);
        IndexError(index_error::Error, index_error::ErrorKind);
    }

    errors {
       /*FontError {
           description("Font could not be read")
           display("Corrupt font file")
       }*/
    }
}