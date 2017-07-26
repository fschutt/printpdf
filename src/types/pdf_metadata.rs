//! Wapper type for shared metadata between XMP Metadata and the DocumentInfo dictionary

extern crate lopdf;
extern crate chrono;

use *;

/// This is a wrapper in order to keep shared data between the documents XMP metadata and
/// the "Info" dictionary in sync
#[derive(Debug, Clone)]
pub struct PdfMetadata {
	/// Creation date of the document
	pub creation_date: chrono::DateTime<chrono::Local>,
	/// Modification date of the document
	pub modification_date: chrono::DateTime<chrono::Local>,
	/// Creation date of the metadata
	pub metadata_date: chrono::DateTime<chrono::Local>,
	/// PDF document title
	pub document_title: String,
	/// Is the document trapped?
	pub trapping: bool,
	/// PDF document version
	pub document_version: u32,
	/// PDF Standard
	pub conformance: PdfConformance,
	/// XMP Metadata. Is ignored on save if the PDF conformance does not allow XMP
	pub xmp_metadata: XmpMetadata,
	/// PDF Info dictionary. Contains metadata for this document
	pub document_info: DocumentInfo,
	/// Target color profile
	pub target_icc_profile: Option<IccProfile>,
}

impl PdfMetadata {

	/// Creates a new metadat object
	pub fn new<S>(title: S, document_version: u32, trapping: bool, conformance: PdfConformance)
	-> Self where S: Into<String>
	{
		let current_time = chrono::Local::now();

		Self {
			creation_date: current_time,
			modification_date: current_time,
			metadata_date: current_time,
			document_title: title.into(),
			trapping: trapping,
			document_version: document_version,
			conformance: conformance,
			xmp_metadata: XmpMetadata::new(Some("default".into()), 1),
			document_info: DocumentInfo::new(),
			target_icc_profile: None,
		}
	}

	/// Consumes the metadata, returning the (xmp_metadata, document_info, icc_profile_stream).
	pub fn into_obj(self)
	-> (lopdf::Object, lopdf::Object, Option<IccProfile>)
	{
		let xmp_obj = self.xmp_metadata.into_obj(self.conformance.clone(), 
												 self.trapping,
												 self.creation_date.clone(), 
												 self.modification_date.clone(), 
												 self.metadata_date.clone(),
												 self.document_title.clone());

		let doc_info_obj = self.document_info.into_obj(self.document_title,
													   self.trapping,
														  self.conformance.clone(),
														  self.creation_date,
														  self.modification_date);
		// add icc profile if necessary
		let icc_profile = {
		    if self.conformance.must_have_icc_profile() {
		        match self.target_icc_profile {
		            Some(icc) => Some(icc),
		            None =>      Some(IccProfile::new(ICC_PROFILE_ECI_V2.to_vec(), IccProfileType::Cmyk)
		            			      .with_alternate_profile(false)
		            			      .with_range(true)),
		        }
		    } else {
		        None
		    }
		};

		(xmp_obj, doc_info_obj, icc_profile)
	}
}
