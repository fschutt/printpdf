extern crate error_chain;
extern crate freetype;
extern crate image;

use super::*;

error_chain! {

    types {
        Error, ErrorKind, ResultExt, Result;
    }

    foreign_links {
        FontFaceError(freetype::Error);
        IoError(::std::io::Error);
    }

    links {
        PDFError(pdf_error::Error, pdf_error::ErrorKind);
        IndexError(index_error::Error, index_error::ErrorKind);
    }

    errors {
       /*Image {
           description("Selected local file is not a PDF file!")
           display("Could not load file")
       }*/
    }
}