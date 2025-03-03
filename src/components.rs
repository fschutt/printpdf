//! HTML components for rendering
use azul_core::{dom::Dom, styled_dom::StyledDom, xml::{normalize_casing, prepare_string, CompileError, ComponentArgumentTypes, ComponentArguments, FilteredComponentArguments, RenderDomError, XmlComponent, XmlComponentMap, XmlComponentTrait, XmlNode, XmlTextContent}};
use azul_css_parser::CssApiWrapper;
use serde_derive::{Serialize, Deserialize};
use crate::{ImageTypeInfo, RawImage, RawImageData, RawImageFormat};

/// Render for a `div` component
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DivRenderer {
    node: XmlNode,
}

impl DivRenderer {
    pub fn new() -> Self {
        Self {
            node: XmlNode::new("div"),
        }
    }
}

impl XmlComponentTrait for DivRenderer {
    fn get_available_arguments(&self) -> ComponentArguments {
        ComponentArguments::default()
    }

    fn render_dom(
        &self,
        _: &XmlComponentMap,
        _: &FilteredComponentArguments,
        _: &XmlTextContent,
    ) -> Result<StyledDom, RenderDomError> {
        Ok(Dom::div().style(CssApiWrapper::empty()))
    }

    fn compile_to_rust_code(
        &self,
        _: &XmlComponentMap,
        _: &ComponentArguments,
        _: &XmlTextContent,
    ) -> Result<String, CompileError> {
        Ok("Dom::div()".into())
    }

    fn get_xml_node(&self) -> XmlNode {
        self.node.clone()
    }
}

/// Render for a `body` component
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BodyRenderer {
    node: XmlNode,
}

impl BodyRenderer {
    pub fn new() -> Self {
        Self {
            node: XmlNode::new("body"),
        }
    }
}

impl XmlComponentTrait for BodyRenderer {
    fn get_available_arguments(&self) -> ComponentArguments {
        ComponentArguments::default()
    }

    fn render_dom(
        &self,
        _: &XmlComponentMap,
        _: &FilteredComponentArguments,
        _: &XmlTextContent,
    ) -> Result<StyledDom, RenderDomError> {
        Ok(Dom::body().style(CssApiWrapper::empty()))
    }

    fn compile_to_rust_code(
        &self,
        _: &XmlComponentMap,
        _: &ComponentArguments,
        _: &XmlTextContent,
    ) -> Result<String, CompileError> {
        Ok("Dom::body()".into())
    }

    fn get_xml_node(&self) -> XmlNode {
        self.node.clone()
    }
}

/// Render for a `p` component
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TextRenderer {
    node: XmlNode,
}

impl TextRenderer {
    pub fn new() -> Self {
        Self {
            node: XmlNode::new("p"),
        }
    }
}

impl XmlComponentTrait for TextRenderer {
    fn get_available_arguments(&self) -> ComponentArguments {
        ComponentArguments {
            args: ComponentArgumentTypes::default(),
            accepts_text: true, // important!
        }
    }

    fn render_dom(
        &self,
        _: &XmlComponentMap,
        _: &FilteredComponentArguments,
        content: &XmlTextContent,
    ) -> Result<StyledDom, RenderDomError> {
        let content = content
            .as_ref()
            .map(|s| prepare_string(&s))
            .unwrap_or_default();
        Ok(Dom::text(content).style(CssApiWrapper::empty()))
    }

    fn compile_to_rust_code(
        &self,
        _: &XmlComponentMap,
        args: &ComponentArguments,
        content: &XmlTextContent,
    ) -> Result<String, CompileError> {
        Ok(String::from("Dom::text(text)"))
    }

    fn get_xml_node(&self) -> XmlNode {
        self.node.clone()
    }
}

/// Render for a `img` component
pub struct ImgComponent {}

#[derive(Debug, Default, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageInfo {
    pub original_id: String,
    pub xobject_id: String,
    pub image_type: ImageTypeInfo,
    pub width: usize,
    pub height: usize,
}

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

pub fn printpdf_default_components() -> XmlComponentMap {
    let mut map = XmlComponentMap {
        components: Vec::new(),
    };
    map.register_component(XmlComponent { 
        id: normalize_casing("body"),
        renderer: Box::new(BodyRenderer::new()), 
        inherit_vars: true 
    });
    map.register_component(XmlComponent { 
        id: normalize_casing("div"),
        renderer: Box::new(DivRenderer::new()), 
        inherit_vars: true 
    });
    map.register_component(XmlComponent { 
        id: normalize_casing("p"),
        renderer: Box::new(TextRenderer::new()), 
        inherit_vars: true 
    });
    map.register_component(XmlComponent {
        id: "img".to_string(),
        renderer: Box::new(ImgComponent { }),
        inherit_vars: false,
    });

    map
}
