use printpdf::*;

const HTML_STRINGS: &[&str; 1] = &["<img src='test.bmp' />"];

pub struct ImgComponent {}

impl XmlComponentTrait for ImgComponent {
    fn get_available_arguments(&self) -> ComponentArguments {
        ComponentArguments {
            accepts_text: false,
            args: vec![("src".to_string(), "String".to_string())],
        }
    }

    fn render_dom(
        &self,
        components: &XmlComponentMap,
        arguments: &FilteredComponentArguments,
        content: &XmlTextContent,
    ) -> Result<StyledDom, RenderDomError> {
        use printpdf::html::ImageInfo;

        let im_info = arguments
            .values
            .get("src")
            .map(|s| s.as_bytes().to_vec())
            .unwrap_or_default();

        let image_info = serde_json::from_slice::<ImageInfo>(&im_info)
            .ok()
            .unwrap_or_default();
        let data_format = RawImageFormat::RGB8;

        let image = RawImage {
            width: image_info.width,
            height: image_info.height,
            data_format,
            pixels: RawImageData::empty(data_format),
            tag: im_info,
        };

        let im = Dom::image(image.to_internal()).style(CssApiWrapper::empty());

        Ok(im)
    }
}

fn main() -> Result<(), String> {
    for (i, h) in HTML_STRINGS.iter().enumerate() {
        let components = vec![XmlComponent {
            id: "img".to_string(),
            renderer: Box::new(ImgComponent {}),
            inherit_vars: false,
        }];

        let config = XmlRenderOptions {
            components,
            images: vec![(
                "test.bmp".to_string(),
                include_bytes!("./assets/img/BMP_test.bmp").to_vec(),
            )]
            .into_iter()
            .collect(),
            ..Default::default()
        };

        let mut doc = PdfDocument::new("HTML rendering demo");
        let pages = doc.html2pages(h, config)?;
        let doc = doc.with_pages(pages).save(&PdfSaveOptions::default());
        std::fs::write(format!("html{i}.pdf"), doc).unwrap();
    }

    Ok(())
}
