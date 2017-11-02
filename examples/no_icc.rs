//! Example to demonstrate how to remove the default ICC profile
//! Look at the file size (compared to the other tests!)

extern crate printpdf;

use printpdf::*;
use std::fs::File;

fn main() {

	/// This code creates the most minimal PDF file with 1.2 KB
	/// Currently, fonts need to use an embedded font, so if you need to write something, the file size
	/// will still be bloated (because of the embedded font)
	/// Also, OCG content is still enabled, even if you disable it here. 
    let (mut doc, _page1, _layer1) = PdfDocument::new("printpdf no_icc test", 297.0, 210.0, "Layer 1");
    doc = doc.with_conformance(PdfConformance::Custom(CustomPdfConformance {
    	requires_icc_profile: false,
    	requires_xmp_metadata: false,
        .. Default::default()
    }));

    doc.save(&mut File::create("test_no_icc.pdf").unwrap()).unwrap();
}