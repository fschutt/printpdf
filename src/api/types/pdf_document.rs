//! A `PDFDocument` represents the whole content of the file

extern crate lopdf;

use super::*;
use super::super::traits::*;
use super::indices::*;

use errors::*;
use api::types::plugins::graphics::two_dimensional::*;
use api::types::plugins::graphics::*;
use std::io::{Write, Seek};

/// PDF document
#[derive(Debug)]
pub struct PdfDocument {
    // Pages of the document
    pages: Vec<PdfPage>,
    /// PDF document title
    title: String,
    /// PDF creator name
    creator: String,
    /// PDF contents (subject to change)
    contents: Vec<Box<IntoPdfObject>>,
    /// Inner PDF document
    inner_doc: lopdf::Document,
}

impl<'a> PdfDocument {

    /// Creates a new PDF document
    #[inline]
    pub fn new<S>(initial_page: PdfPage, title: S, creator: S)
    -> (Self, PdfPageIndex, PdfLayerIndex) where S: Into<String>
    {
        (Self {
            pages: vec![initial_page],
            title: title.into(),
            creator: creator.into(),
            contents: Vec::new(),
            inner_doc: lopdf::Document::new(),
        },
        PdfPageIndex(0),
        PdfLayerIndex(0))
    }

    /// # `add_*` functions

    /// Create a new pdf page and returns the index of the page
    #[inline]
    pub fn add_page(&mut self, x_mm: f64, y_mm: f64, inital_layer: PdfLayer)
    -> (PdfPageIndex, PdfLayerIndex)
    {
        self.pages.push(PdfPage::new(x_mm, y_mm, inital_layer));
        (PdfPageIndex(self.pages.len() - 1), PdfLayerIndex(0))
    }

    /// Create a new pdf layer on the given page and returns the index of the new layer
    #[inline]
    pub fn add_layer(&mut self, page: PdfPageIndex, added_layer: PdfLayer)
    -> ::std::result::Result<PdfLayerIndex, Error>
    {
        use errors::index_error::ErrorKind::*;
        let layer_index = self.pages.get_mut(page.0)
                              .ok_or(Error::from_kind(IndexError(PdfPageIndexError))).unwrap()
                              .add_layer(added_layer);
        Ok(layer_index)
    }

    /// Add arbitrary Pdf Objects. These are tracked by reference and get 
    /// instantiated / referenced when the document is saved.
    #[inline]
    pub fn add_arbitrary_content<C>(&mut self, content: Box<C>)
    -> PdfContentIndex where C: 'static + IntoPdfObject
    {
        self.contents.push(content);
        PdfContentIndex(self.contents.len() - 1)
    }

    /// ## `add_*` functions for arbitrary PDF content

    /// Add a font from a 
    #[inline]
    pub fn add_font<R>(&mut self, font_stream: R)
    -> ::std::result::Result<FontIndex, Error> where R: ::std::io::Read
    {
        use api::types::plugins::graphics::two_dimensional::Font;
        let font = Font::new(font_stream)?;
        let index = self.add_arbitrary_content(Box::new(font));
        Ok(FontIndex(index))
    }

    /// Add text to the file
    #[inline]
    pub fn add_text<S>(&mut self, 
                      text: S, 
                      font: FontIndex, 
                      font_size: usize,
                      x_mm: f64,
                      y_mm: f64,
                      layer: PdfLayerIndex)
    -> ::std::result::Result<(), Error> where S: Into<String>
    {
        // todo
        Ok(())
    }

    /// Add a line to the document
    #[inline]
    pub fn add_line(&mut self,
                    points: Vec<(Point, bool)>, 
                    outline: Option<&Outline>, 
                    fill: Option<&Fill>,
                    layer: PdfLayerIndex)
    -> ::std::result::Result<(), Error>
    {
        // todo
        Ok(())
    }

    /// Add SVG content to the document
    #[inline]
    pub fn add_svg<R>(&mut self,
                      svg_data: R)
    -> ::std::result::Result<SvgIndex, Error> 
    where R: ::std::io::Read
    {
        // todo
        unimplemented!()
    }

    /// Instantiate SVG data
    #[inline]
    pub fn add_svg_at(&mut self,
                      svg_data_index: SvgIndex,
                      width_mm: f64,
                      height_mm: f64,
                      x_mm: f64,
                      y_mm: f64,
                      layer: PdfLayerIndex)
    {
        // todo
    }

    /// # `get_*` functions

    /// Validates that a page is accessible and returns the page index
    #[inline]
    pub fn get_page(&self, page: usize)
    -> ::std::result::Result<PdfPageIndex, Error>
    {
        use errors::index_error::ErrorKind::*;
        let index = self.pages.get(page)
                              .ok_or(Error::from_kind(IndexError(PdfPageIndexError)));
        Ok(PdfPageIndex(page))
    }

    /// Drops the PDFDocument, returning the inner `lopdf::Document`. 
    /// Document may be only half-written
    #[inline]
    pub unsafe fn get_inner(self)
    -> (lopdf::Document, Vec<Box<IntoPdfObject>>)
    {
        (self.inner_doc, self.contents)
    }

    /// ## Miscellaneous functions

    /// Save PDF Document, writing the contents to the target
    pub fn save<W: Write + Seek>(mut self, target: &mut W)
    -> ::std::result::Result<(), Error>
    {
        use lopdf::{Dictionary, Object};
        use lopdf::Object::{Integer, Reference};
        use std::iter::FromIterator;

        // add root, catalog & pages
        let pages_id = self.inner_doc.new_object_id();

        let catalog = Dictionary::from_iter(vec![
                      ("Type", "Catalog".into()),
                      ("PageLayout", "OneColumn".into()),
                      ("PageMode", "Use0".into()),
                      ("Pages", Reference(pages_id)), ]);

        let mut pages = Dictionary::from_iter(vec![
                      ("Type", "Pages".into()),
                      ("Count", Integer(self.pages.len() as i64)),
                      /* Kids and Resources missing */
                      ]);

        // add all contents, save references
        // todo

        // add pages
        let mut page_ids = Vec::<Object>::new();

        for page in self.pages.clone().into_iter() {
            
            let p = Dictionary::from_iter(vec![
                      ("Type", "Page".into()),
                      ("MediaBox", vec![0.into(), 0.into(),
                       page.width_pt.into(), page.heigth_pt.into()].into()),
                      ("Parent", Reference(pages_id)),
                      /* todo: ArtBox */ ]);

            // add page content references
            // todo

            page_ids.push(Reference(self.inner_doc.add_object(p)))
        }

        pages.set::<String, Object>("Kids".into(), page_ids.into());
        self.inner_doc.objects.insert(pages_id, Object::Dictionary(pages));

        // save inner document
        let catalog_id = self.inner_doc.add_object(catalog);
        self.inner_doc.trailer.set("Root", Reference(catalog_id));
        self.inner_doc.compress();
        self.inner_doc.save_to(target).unwrap();

        Ok(())
    }
}

/*
impl std::convert::From<lopdf::Doument> for PdfDocument
{
    fn from(doc: lopdf::Doument) -> Self
    {
        
    }
}
*/