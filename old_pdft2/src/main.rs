//! Crate for exporting a map to a PDF
extern crate lopdf;
extern crate unicode_normalization as unic;
extern crate lcms2;
extern crate freetype as ft;

use lopdf::{Document, Object, ObjectId, Dictionary, Stream, StringFormat};
use lopdf::content::{Content, Operation};
use Object::*;
use std::iter::FromIterator;
use std::str;
use std::collections::{HashMap, BTreeMap};
use std::io::prelude::*;
use std::fs::File;
use std::string::String;

const FONT_PATH: &'static str =  "FreeSans.ttf";
const ICC_PATH: &'static str = "ISOcoated_v2_300_eci.icc";


/// Converts points into millimeter
macro_rules! pt {
	($mm: expr) => ($mm * 2.834646);
}

/// Document used for creating PDF map documents
pub struct MapDocument {
	doc: 			Document,
	catalog: 		Dictionary,
	pages: 			Dictionary,
	page: 			Dictionary,
	content: 		Vec<Content>, /* TODO: content should be a vector of layers */
	resources:		Dictionary,
	objects:		Vec<Object>,
	fonts:			HashMap<String, Dictionary>,
}

// Examples of how points, lines and Polygons may be structured
// The line has a boolean value to indicate if the next point should use arcTo instead of lineTo
#[derive(Debug, Copy, Clone)]
pub struct Point { x: f64, y: f64 }
#[derive(Debug, Clone)]
pub struct Line { points: Vec<(Point, bool)> }
#[derive(Debug, Clone)]
pub struct Polygon { lines: Vec<Line> }
/// CMYK color
#[derive(Debug, Clone)]
pub struct CMYK{ c: f64, m: f64, y: f64, k: f64, }

impl PartialEq for Point {
	// custom compare function because of floating point inaccuracy
    fn eq(&self, other: &Point) -> bool {
        if self.x.is_normal() && other.x.is_normal() &&
		   self.y.is_normal() && other.y.is_normal() {
			// four floating point numbers have to match
			let x_eq = (self.x * 1000.0).round() == (other.x * 1000.0).round();
			if !x_eq { return false; }
			let y_eq = (self.y * 1000.0).round() == (other.y * 1000.0).round();
			if y_eq { return true; }
		}
		return false;
    }
}


impl MapDocument {

	//----------------- LEVEL 1 ABSTRATION OVER IOPDF ----------------//
	//---- DO NOT USE THESE FUNCTIONS DIRECTLY UNLESS YOU HAVE TO ----//

	/// Create a new document with one page
	pub fn new(width: u32, height: u32, icc_path: &str) -> Result<MapDocument, std::io::Error> {

		let width_f_mm = width as f64;
		let height_f_mm = height as f64;

		let mut doc = Document::new();
		doc.version = "1.4".to_string();

		// Create the necessary dictionaries
		// They can be pushed to and queried - they are only written to the document object on save
		let pages = 	Dictionary::from_iter(vec![
		    			("Type", "Pages".into()),
		    			("Count", 1.into()),
						("MediaBox", vec![0.into(), 0.into(), pt!(width_f_mm).into(), pt!(height_f_mm).into()].into()),
						/* Kids and Resources missing */
						]);

		let catalog = 	Dictionary::from_iter(vec![
					  	("Type", "Catalog".into()),
						("PageLayout", "OneColumn".into()),
						("PageMode", "Use0".into()),
						]);

		let page = 		Dictionary::from_iter(vec![
						("Type", "Page".into()),
			    		]);

		let objects	  	= Vec::<Object>::new();
		let mut content = Vec::<Content>::new();
		content.push( Content{ operations: Vec::<Operation>::new()} );

		content[0].operations.push(Operation::new("cs", vec![Name("ISOcoated_v2_300_eci".into())] ));
		content[0].operations.push(Operation::new("CS", vec![Name("ISOcoated_v2_300_eci".into())] ));

		// Start embedding color profile in resources
		let mut icc_ref: std::option::Option<Object> = None;

		if let Ok(mut icc_profile_file_handle) = File::open(icc_path){
			let mut icc_buf = Vec::<u8>::new();
			if let Ok(_) = icc_profile_file_handle.read_to_end(&mut icc_buf){
				let icc = Stream::new(Dictionary::from_iter(vec![
						("N", Integer(4)).into(),
						("Alternate", Name("DeviceCMYK".into())).into(),
						("Length", Integer(icc_buf.len() as i64).into())]),
					icc_buf
				);
				icc_ref = Some(Reference(doc.add_object(Stream(icc))));
			}
		}

		let mut resources = Dictionary::new();

		if let Some(icc_reference) = icc_ref {
			resources = Dictionary::from_iter(vec![
					("ColorSpace", Dictionary(Dictionary::from_iter(vec![
							/*usually ISO Coated v2 300 ECI*/
							("ISOcoated_v2_300_eci", Array(vec![Name("ICCBased".into()), icc_reference])),
						])),
				)]);

		} else {
			println!("WARNING: Could not embed color profile");
		}
		// End embedding color profile into resources

		Ok(MapDocument {
			doc: doc,
			pages: pages,
			content: content,
			catalog: catalog,
			page: page,
			resources: resources,
			objects: objects,
			fonts: HashMap::<String, Dictionary>::new(),
		})
	}

	/// Saves a MapDocument while writing all the necessary objects
	pub fn save_map(mut self, path: &str) {

		// Write root element
		let start = self.doc.new_object_id();
		self.catalog.set("Pages", Reference(start));
		self.page.set("Parent", Reference(start));

		// Write layers
		let content_id = self.doc.add_object(Stream::new(Dictionary::new(), self.content[0].encode().unwrap()));
	  	self.page.set::<String, Object>("Contents".into(), vec![Reference(content_id)].into());

		// Create font dictionary
		let mut font_dict_inner = Dictionary::new();
		for (key, val) in self.fonts {
			font_dict_inner.set(key, Reference(self.doc.add_object(val)));
		}
		self.resources.set::<String, Dictionary>("Font".into(), font_dict_inner);
		self.pages.set::<String, Dictionary>("Resources".into(), self.resources);

		// Write page
		let page_id = self.doc.add_object(self.page);
		self.pages.set::<String, Object>("Kids".into(), vec![Reference(page_id)].into());
		self.doc.objects.insert(start, Object::Dictionary(self.pages));
		let catalog_id = self.doc.add_object(self.catalog);

		self.doc.trailer.set("Root", Reference(catalog_id));
		self.doc.compress();
		self.doc.save(path).unwrap();
	}

	/// Writes a free object into the PDF. Resources cannot be referenced or reused,
	/// they are "final content"
	pub fn add_object(&mut self, d: Dictionary) -> ObjectId {
		let object_id = self.doc.add_object(d);
		self.objects.push(Reference(object_id.clone()));
		object_id
	}

	/// Takes a vector of graphical operations and pushes it into self.content
	#[inline]
	pub fn push(&mut self, key: &str, value: Vec<Object>){
		self.content[0].operations.push(Operation::new(key, value));
	}

	/// Takes a font and embeds it into the document as a resource
	/// The font can later be referred to it by the "nice name"
	pub fn add_ttf(&mut self, path: &str) -> Result<String, std::io::Error>{

		// Load font
		let mut font_buf = Vec::<u8>::new();
		let mut font_file = File::open(path).expect("L218");
		let _ = font_file.read_to_end(&mut font_buf);
		let font_buf_ref: Box<[u8]> = font_buf.into_boxed_slice();
		let library = ft::Library::init().unwrap();
	    let face = library.new_memory_face(&*font_buf_ref, 0).unwrap();

		// Extract basic font information
		// TODO: return specific error when returning
		let face_name = face.postscript_name().unwrap();
		let face_metrics = face.size_metrics().unwrap();

		let font_stream = Stream::new(
			Dictionary::from_iter(vec![
				/*("Length1", Integer(font_buf_ref.len() as i64)),*/
				("Subtype", Name("CIDFontType0C".into())),
				]),
			font_buf_ref.to_vec());

		// Begin setting required font attributes
		let mut font_vec: Vec<(String, Object)> = vec![
			("Type".into(), Name("Font".into())),
			("Subtype".into(), Name("Type0".into())),
			("BaseFont".into(), Name(face_name.clone())),
			("Encoding".into(), Name("Identity-H".into())),
		];

		let mut font_descriptor_vec: Vec<(String, Object)> = vec![
			("Type".into(), Name("FontDescriptor".into())),
			("FontName".into(), Name(face_name.clone())),
			("Ascent".into(), Integer(face_metrics.ascender)),
			("Descent".into(), Integer(face_metrics.descender)),
			("CapHeight".into(), Integer(face_metrics.ascender)),
			("ItalicAngle".into(), Integer(0)),
			("Flags".into(), Integer(32)),
			("StemV".into(), Integer(80)),
		];
		// End setting required font arguments

		let mut max_height = 0;				// Maximum height of the font
		let mut total_width = 0;			// Total width of all characters
		let mut widths = Vec::<Object>::new();			   // Widths of the individual characters
		let mut cmap = BTreeMap::<u32, (u32, u32)>::new(); // Glyph IDs - (Unicode IDs - character width)
		cmap.insert(0, (0, 1000));

		for unicode in 0x0000..0xffff {
			let glyph_id = face.get_char_index(unicode);
			if glyph_id != 0 {
				// this should not fail - if we can get the glyph id, we can get the glyph itself
				if face.load_glyph(glyph_id, ft::face::NO_SCALE).is_ok() {
					let glyph_slot = face.glyph();
					let glyph_metrics = glyph_slot.metrics();

					if glyph_metrics.height > max_height{
						max_height = glyph_metrics.height;
					};

					total_width += glyph_metrics.width;
					cmap.insert(glyph_id, (unicode as u32, glyph_metrics.width as u32));
				}
			}
		}

		// Maps the character index to a unicode value
		// Add this to the "ToUnicode" dictionary
		// To explain this structure: Glyph IDs have to be in segments where the first byte of the
		// first and last element have to be the same. A range from 0x1000 - 0x10FF is valid
		// but a range from 0x1000 - 0x12FF is not (0x10 != 0x12)
		// Plus, the maximum number of Glyph-IDs in one range is 100
		// Since the glyph IDs are sequential, all we really have to do is to enumerate the vector
		// and create buckets of 100 / rest to 256 if needed
		let mut cid_to_unicode_map = format!(include_str!("gid_to_unicode_beg.txt"), face_name.clone());

		let mut cur_block_id: u32 = 0;			// ID of the block, to be used it {} beginbfchar
		let mut cur_first_bit: u16 = 0_u16;		// current first bit of the glyph id (0x10 or 0x12) for example
		let mut last_block_begin: u32 = 0;		// glyph ID of the start of the current block,
												// to satisfy the "less than 100 entries per block" rule

		for (glyph_id, unicode_width_tuple) in cmap.iter() {

			if (*glyph_id >> 8) as u16 != cur_first_bit || *glyph_id > last_block_begin + 100 {
				cid_to_unicode_map.push_str("endbfchar\r\n");
				cur_block_id += 1;
				last_block_begin = *glyph_id;
				cur_first_bit = (*glyph_id >> 8) as u16;
				cid_to_unicode_map.push_str(format!("{} beginbfchar\r\n", cur_block_id).as_str());
			}

			let unicode = unicode_width_tuple.0;
			let width = unicode_width_tuple.1;
			cid_to_unicode_map.push_str(format!("<{:04x}> <{:04x}>\n", glyph_id, unicode).as_str());
			widths.push(Integer(width as i64));
		};

		if cmap.len() % 256 != 0 || cmap.len() % 100 != 0 {
			cid_to_unicode_map.push_str("endbfchar\r\n");
		}

		cid_to_unicode_map.push_str(include_str!("gid_to_unicode_end.txt"));
		let cid_to_unicode_map_stream = Stream::new(Dictionary::new(), cid_to_unicode_map.as_bytes().to_vec());

		let mut desc_fonts = Dictionary::from_iter(vec![
			("Type", Name("Font".into())),
			("Subtype", Name("CIDFontType0".into())),
			("BaseFont", Name(face_name.clone().into())),
			("W",  Array(vec![Integer(1), Array(widths)])),
			("CIDSystemInfo", Dictionary(Dictionary::from_iter(vec![
					("Registry", String("Adobe".into(), StringFormat::Literal)),
					("Ordering", String("Identity".into(), StringFormat::Literal)),
					("Supplement", Integer(0)),
			]))),
			/*("CIDToGIDMap", Reference(*cid_system_info_id)),*/
		]);

		let font_bbox = vec![ Integer(0), Integer(max_height), Integer(total_width), Integer(max_height) ];
		font_descriptor_vec.push(("FontBBox".into(), Array(font_bbox)));

		let font_stream_id = &self.doc.add_object(font_stream);
		font_descriptor_vec.push(("FontFile3".into(), Reference(*font_stream_id)));

		// Create dictionaries and add to DOM
		let font_descriptor_id = &self.doc.add_object(Dictionary::from_iter(font_descriptor_vec));
		desc_fonts.set("FontDescriptor".to_string(), Reference(*font_descriptor_id));

		// Embed character ids
		let cid_to_unicode_map_stream_id = &self.doc.add_object(Stream(cid_to_unicode_map_stream));
		font_vec.push(("ToUnicode".into(), Reference(*cid_to_unicode_map_stream_id)));
		// let char_to_cid_map_stream_id = &self.doc.add_object(Stream(char_to_cid_map_stream));
		// font_vec.push(("Encoding".into(), Name("Identity-H".into())));

		let desc_fonts_id = &self.doc.add_object(Array(vec![Dictionary(desc_fonts)]));
		font_vec.push(("DescendantFonts".into(), Reference(*desc_fonts_id)));

		let font = Dictionary::from_iter(font_vec);
		&self.fonts.insert(face_name.clone(), font);

		Ok(face_name)
	}

	//------------ LEVEL 2 ABSTRATION -----------//
	/// Add text to document
	pub fn add_text(&mut self, string: &str, size_pt: i64, position: &Point, font: String) {
		// take utf8 string, encode to utf16be
		// convert to glyph ID
		// Load font
		// REMOVE THIS!!
		let mut font_buf = Vec::<u8>::new();
		let mut font_file = File::open(FONT_PATH).expect("L366");
		let _ = font_file.read_to_end(&mut font_buf);
		let font_buf_ref: Box<[u8]> = font_buf.into_boxed_slice();
		let library = ft::Library::init().unwrap();
	    let face = library.new_memory_face(&*font_buf_ref, 0).unwrap();

		let str_test = string.to_string();
		let list_gid: Vec<u16> = str_test.chars().map(|x| face.get_char_index(x as usize) as u16).collect();
		// let str: Vec<u16> = string.encode_utf16().collect();
		let bytes: Vec<u8> = list_gid.iter()
			.flat_map(|x| vec!((x >> 8) as u8, (x & 255) as u8))
			.collect::<Vec<u8>>();

		// rotation missing
		&self.push("BT", vec![]);
		&self.push("Tf", vec![font.into(), size_pt.into()]);
		&self.push("Td", vec![position.x.into(), position.y.into()]);
		&self.push("Tj", vec![Object::String(bytes, StringFormat::Hexadecimal)]);
		&self.push("ET", vec![]);
	}

	/// Draw a line
	/// TODO: support resetting of outline to
	pub fn draw_line(&mut self,
					 line: &Line,
					 outline_col: Option<&CMYK>,
					 outline_pt: Option<i64>,
					 fill_col: Option<&CMYK>)
	{
		if line.points.is_empty() { return; };

		// Set color space and width
		outline_col.map(|c| self.push("SCN", vec![c.c.into(), c.m.into(), c.y.into(), c.k.into()]));
		fill_col.map(|c| self.push("scn", vec![c.c.into(), c.m.into(), c.y.into(), c.k.into()]));
		outline_pt.map(|w| self.push("w", vec![w.into()]));

		self.push("m", vec![line.points[0].0.x.into(), line.points[0].0.y.into()]);

		// Skip first element
		let mut current = 1;
		let max_len = line.points.len();

		// Loop over every points, determine if v, y, c or l operation should be used and build
		// curve / line accordingly
		while current < max_len {
			let p1 = &line.points[current - 1];						 // prev pt
			let p2 = &line.points[current];							 // current pt

			if p1.1 && p2.1 {
				// current point is a bezier handle
				// valid bezier curve must have two sequential bezier handles
				// we also can"t build a valid cubic bezier curve if the cuve contains less than
				// four points. If p3 or p4 is marked as "next point is bezier handle" or not, doesn"t matter
				if let Some(p3) = line.points.get(current + 1) {
					if let Some(p4) = line.points.get(current + 2){
						if p1.0 == p2.0 {
							// first control point coincides with initial point of curve
							self.push("v", vec![p3.0.x.into(), p3.0.y.into(), p4.0.x.into(), p4.0.y.into()]);
						}else if p2.0 == p3.0 {
							// first control point coincides with final point of curve
							self.push("y", vec![p2.0.x.into(), p2.0.y.into(), p4.0.x.into(), p4.0.y.into()]);
						}else{
							// regular bezier curve with four points
							self.push("c", vec![p2.0.x.into(), p2.0.y.into(), p3.0.x.into(), p3.0.y.into(), p4.0.x.into(), p4.0.y.into()]);
						}
						current += 3;
						continue;
					}
				}
			}

			// normal straight line
			self.push("l", vec![p2.0.x.into(), p2.0.y.into()]);
			current += 1;
		}

		//todo set color beforehand
		match fill_col {
			Some(_) => {self.push("b", vec![]);},
			None	=> {self.push("S", vec![]);},
		}
	}

}

fn main() {

   	let mut doc = MapDocument::new(1000, 1000, ICC_PATH).unwrap();

	let roboto_font_ref = doc.add_ttf(FONT_PATH).unwrap();

	// line drawing
	let a = Point { x: 200.0, y: 50.0 };
	let b = Point { x: 500.0, y: 295.0 };
	let c = Point { x: 300.0, y: 26.0 };
	let d = Point { x: 400.0, y: 560.0 };
	let e = Point { x: 900.0, y: 29.0 };
	let f = Point { x: 150.0, y: 650.0 };

	// todo: make creation functions for this
	let line = Line { points: vec![(a, true), (a, true), (b, true), (c, false), (d, false), (e, false), (f, false)], };
	let color = CMYK { c: 1.0, m: 0.75, y: 0.0, k: 0.0 };
	doc.draw_line(&line, Some(&color), Some(10), Some(&color));

	// text drawing
	let text_pos = Point { x: 10.0, y: 40.0, };
	doc.add_text("стуфхfцчшщъыьэюя", 48, &text_pos, roboto_font_ref);

	doc.save_map("example_6.pdf");
}
