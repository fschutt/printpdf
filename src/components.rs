use azul_core::{
    dom::Dom,
    styled_dom::StyledDom,
    xml::{
        CompileError, ComponentArgumentTypes, ComponentArguments, FilteredComponentArguments,
        RenderDomError, XmlComponent, XmlComponentMap, XmlComponentTrait, XmlNode, XmlTextContent,
        normalize_casing, prepare_string,
    },
};
use azul_css_parser::CssApiWrapper;
use serde_derive::{Deserialize, Serialize};

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

/// Text renderer for p, h1-h6, strong and em elements
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TextRenderer {
    node: XmlNode,
    css: String,
}

impl TextRenderer {
    pub fn new() -> Self {
        Self {
            node: XmlNode::new("p"),
            css: String::new(),
        }
    }
    
    pub fn with_css(node_type: &str, css: &str) -> Self {
        Self {
            node: XmlNode::new(node_type),
            css: css.to_string(),
        }
    }
}

impl XmlComponentTrait for TextRenderer {
    fn get_available_arguments(&self) -> ComponentArguments {
        ComponentArguments {
            args: ComponentArgumentTypes::default(),
            accepts_text: true,
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
        
        if self.css.is_empty() {
            Ok(Dom::text(content).style(CssApiWrapper::empty()))
        } else {
            Ok(Dom::text(content).style(CssApiWrapper::from_string(self.css.clone().into())))
        }
    }

    fn compile_to_rust_code(
        &self,
        _: &XmlComponentMap,
        _: &ComponentArguments,
        _: &XmlTextContent,
    ) -> Result<String, CompileError> {
        Ok(String::from("Dom::text(text)"))
    }

    fn get_xml_node(&self) -> XmlNode {
        self.node.clone()
    }
}

/// Renderer for horizontal rule (hr) element
pub struct HrRenderer {
    node: XmlNode,
}

impl HrRenderer {
    pub fn new() -> Self {
        Self {
            node: XmlNode::new("hr"),
        }
    }
}

impl XmlComponentTrait for HrRenderer {
    fn get_available_arguments(&self) -> ComponentArguments {
        ComponentArguments::default()
    }

    fn render_dom(
        &self,
        _: &XmlComponentMap,
        _: &FilteredComponentArguments,
        _: &XmlTextContent,
    ) -> Result<StyledDom, RenderDomError> {
        Ok(Dom::div().style(CssApiWrapper::from_string(
            "border: none; height: 1px; background-color: #ccc; margin: 10px 0; width: 100%;".into()
        )))
    }

    fn compile_to_rust_code(
        &self,
        _: &XmlComponentMap,
        _: &ComponentArguments,
        _: &XmlTextContent,
    ) -> Result<String, CompileError> {
        Ok(String::from("Dom::div()"))
    }

    fn get_xml_node(&self) -> XmlNode {
        self.node.clone()
    }
}

/// Renderer for table element
pub struct TableRenderer {
    node: XmlNode,
}

impl TableRenderer {
    pub fn new() -> Self {
        Self {
            node: XmlNode::new("table"),
        }
    }
}

impl XmlComponentTrait for TableRenderer {
    fn get_available_arguments(&self) -> ComponentArguments {
        ComponentArguments::default()
    }

    fn render_dom(
        &self,
        _: &XmlComponentMap,
        _: &FilteredComponentArguments,
        _: &XmlTextContent,
    ) -> Result<StyledDom, RenderDomError> {
        Ok(Dom::div().style(CssApiWrapper::from_string(
            "display: flex; flex-direction: column; border: 1px solid #ccc; width: 100%;".into()
        )))
    }

    fn compile_to_rust_code(
        &self,
        _: &XmlComponentMap,
        _: &ComponentArguments,
        _: &XmlTextContent,
    ) -> Result<String, CompileError> {
        Ok(String::from("Dom::div()"))
    }

    fn get_xml_node(&self) -> XmlNode {
        self.node.clone()
    }
}

/// Renderer for table row (tr) element
pub struct TrRenderer {
    node: XmlNode,
}

impl TrRenderer {
    pub fn new() -> Self {
        Self {
            node: XmlNode::new("tr"),
        }
    }
}

impl XmlComponentTrait for TrRenderer {
    fn get_available_arguments(&self) -> ComponentArguments {
        ComponentArguments::default()
    }

    fn render_dom(
        &self,
        _: &XmlComponentMap,
        _: &FilteredComponentArguments,
        _: &XmlTextContent,
    ) -> Result<StyledDom, RenderDomError> {
        Ok(Dom::div().style(CssApiWrapper::from_string(
            "display: flex; flex-direction: row; width: 100%;".into()
        )))
    }

    fn compile_to_rust_code(
        &self,
        _: &XmlComponentMap,
        _: &ComponentArguments,
        _: &XmlTextContent,
    ) -> Result<String, CompileError> {
        Ok(String::from("Dom::div()"))
    }

    fn get_xml_node(&self) -> XmlNode {
        self.node.clone()
    }
}

/// Renderer for table header (th) element
pub struct ThRenderer {
    node: XmlNode,
}

impl ThRenderer {
    pub fn new() -> Self {
        Self {
            node: XmlNode::new("th"),
        }
    }
}

impl XmlComponentTrait for ThRenderer {
    fn get_available_arguments(&self) -> ComponentArguments {
        ComponentArguments {
            args: ComponentArgumentTypes::default(),
            accepts_text: true,
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
        
        let css = "padding: 8px; border: 1px solid #ccc; font-weight: bold; text-align: left; flex: 1;";
        let mut dom = Dom::div().style(CssApiWrapper::from_string(css.into()));
        
        if !content.is_empty() {
            dom.append_child(Dom::text(content).style(CssApiWrapper::empty()));
        }
        
        Ok(dom)
    }

    fn compile_to_rust_code(
        &self,
        _: &XmlComponentMap,
        _: &ComponentArguments,
        _: &XmlTextContent,
    ) -> Result<String, CompileError> {
        Ok(String::from("Dom::div()"))
    }

    fn get_xml_node(&self) -> XmlNode {
        self.node.clone()
    }
}

/// Renderer for table data (td) element
pub struct TdRenderer {
    node: XmlNode,
}

impl TdRenderer {
    pub fn new() -> Self {
        Self {
            node: XmlNode::new("td"),
        }
    }
}

impl XmlComponentTrait for TdRenderer {
    fn get_available_arguments(&self) -> ComponentArguments {
        ComponentArguments {
            args: ComponentArgumentTypes::default(),
            accepts_text: true,
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
        
        let css = "padding: 8px; border: 1px solid #ccc; flex: 1;";
        let mut dom = Dom::div().style(CssApiWrapper::from_string(css.into()));
        
        if !content.is_empty() {
            dom.append_child(Dom::text(content).style(CssApiWrapper::empty()));
        }
        
        Ok(dom)
    }

    fn compile_to_rust_code(
        &self,
        _: &XmlComponentMap,
        _: &ComponentArguments,
        _: &XmlTextContent,
    ) -> Result<String, CompileError> {
        Ok(String::from("Dom::div()"))
    }

    fn get_xml_node(&self) -> XmlNode {
        self.node.clone()
    }
}

/// Renderer for unordered list (ul) element
pub struct UlRenderer {
    node: XmlNode,
}

impl UlRenderer {
    pub fn new() -> Self {
        Self {
            node: XmlNode::new("ul"),
        }
    }
}

impl XmlComponentTrait for UlRenderer {
    fn get_available_arguments(&self) -> ComponentArguments {
        ComponentArguments::default()
    }

    fn render_dom(
        &self,
        _: &XmlComponentMap,
        _: &FilteredComponentArguments,
        _: &XmlTextContent,
    ) -> Result<StyledDom, RenderDomError> {
        Ok(Dom::div().style(CssApiWrapper::from_string(
            "display: flex; flex-direction: column; padding-left: 20px; margin: 16px 0;".into()
        )))
    }

    fn compile_to_rust_code(
        &self,
        _: &XmlComponentMap,
        _: &ComponentArguments,
        _: &XmlTextContent,
    ) -> Result<String, CompileError> {
        Ok(String::from("Dom::div()"))
    }

    fn get_xml_node(&self) -> XmlNode {
        self.node.clone()
    }
}

/// Renderer for ordered list (ol) element
pub struct OlRenderer {
    node: XmlNode,
}

impl OlRenderer {
    pub fn new() -> Self {
        Self {
            node: XmlNode::new("ol"),
        }
    }
}

impl XmlComponentTrait for OlRenderer {
    fn get_available_arguments(&self) -> ComponentArguments {
        ComponentArguments::default()
    }

    fn render_dom(
        &self,
        _: &XmlComponentMap,
        _: &FilteredComponentArguments,
        _: &XmlTextContent,
    ) -> Result<StyledDom, RenderDomError> {
        Ok(Dom::div().style(CssApiWrapper::from_string(
            "display: flex; flex-direction: column; padding-left: 20px; margin: 16px 0;".into()
        )))
    }

    fn compile_to_rust_code(
        &self,
        _: &XmlComponentMap,
        _: &ComponentArguments,
        _: &XmlTextContent,
    ) -> Result<String, CompileError> {
        Ok(String::from("Dom::div()"))
    }

    fn get_xml_node(&self) -> XmlNode {
        self.node.clone()
    }
}

/// Type of list for list items
pub enum ListType {
    Unordered,
    Ordered,
}

/// Renderer for list item (li) element
pub struct LiRenderer {
    node: XmlNode,
    list_type: ListType,
}

impl LiRenderer {
    pub fn new() -> Self {
        Self {
            node: XmlNode::new("li"),
            list_type: ListType::Unordered,
        }
    }
    
    pub fn with_list_type(list_type: ListType) -> Self {
        Self {
            node: XmlNode::new("li"),
            list_type,
        }
    }
}

impl XmlComponentTrait for LiRenderer {
    fn get_available_arguments(&self) -> ComponentArguments {
        ComponentArguments {
            args: ComponentArgumentTypes::default(),
            accepts_text: true,
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
        
        // Main container div for the list item (with flexbox row)
        let mut item_container = Dom::div().style(CssApiWrapper::from_string(
            "display: flex; flex-direction: row; align-items: flex-start; margin: 4px 0;".into()
        ));
        
        // Bullet point div
        let bullet_text = match self.list_type {
            ListType::Unordered => "•",
            ListType::Ordered => "1.",  // This would ideally be a counter in real CSS
        };
        
        let bullet_div = Dom::div()
        .with_child(Dom::text(bullet_text))
        .style(CssApiWrapper::from_string(
            "width: 20px; flex-shrink: 0; text-align: center; margin-right: 5px;".into()
        ));
        
        // Content div
        let mut content_div = Dom::div().style(CssApiWrapper::from_string(
            "flex-grow: 1;".into()
        ));
        
        if !content.is_empty() {
            content_div.append_child(Dom::text(content).style(CssApiWrapper::empty()));
        }
        
        // Add the bullet and content divs to the container
        item_container.append_child(bullet_div);
        item_container.append_child(content_div);
        
        Ok(item_container)
    }

    fn compile_to_rust_code(
        &self,
        _: &XmlComponentMap,
        _: &ComponentArguments,
        _: &XmlTextContent,
    ) -> Result<String, CompileError> {
        Ok(String::from("Dom::div()"))
    }

    fn get_xml_node(&self) -> XmlNode {
        self.node.clone()
    }
}

/// Renderer for strong (bold text) element
pub struct StrongRenderer {
    node: XmlNode,
}

impl StrongRenderer {
    pub fn new() -> Self {
        Self {
            node: XmlNode::new("strong"),
        }
    }
}

impl XmlComponentTrait for StrongRenderer {
    fn get_available_arguments(&self) -> ComponentArguments {
        ComponentArguments {
            args: ComponentArgumentTypes::default(),
            accepts_text: true,
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
            
        Ok(Dom::text(content).style(CssApiWrapper::from_string("font-weight: bold;".into())))
    }

    fn compile_to_rust_code(
        &self,
        _: &XmlComponentMap,
        _: &ComponentArguments,
        _: &XmlTextContent,
    ) -> Result<String, CompileError> {
        Ok(String::from("Dom::text(text)"))
    }

    fn get_xml_node(&self) -> XmlNode {
        self.node.clone()
    }
}

/// Renderer for em (italic text) element
pub struct EmRenderer {
    node: XmlNode,
}

impl EmRenderer {
    pub fn new() -> Self {
        Self {
            node: XmlNode::new("em"),
        }
    }
}

impl XmlComponentTrait for EmRenderer {
    fn get_available_arguments(&self) -> ComponentArguments {
        ComponentArguments {
            args: ComponentArgumentTypes::default(),
            accepts_text: true,
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
            
        Ok(Dom::text(content).style(CssApiWrapper::from_string("font-style: italic;".into())))
    }

    fn compile_to_rust_code(
        &self,
        _: &XmlComponentMap,
        _: &ComponentArguments,
        _: &XmlTextContent,
    ) -> Result<String, CompileError> {
        Ok(String::from("Dom::text(text)"))
    }

    fn get_xml_node(&self) -> XmlNode {
        self.node.clone()
    }
}

/// Render for an `img` component
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
        _: &XmlComponentMap,
        arguments: &FilteredComponentArguments,
        _: &XmlTextContent,
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
    
    // Register base elements
    map.register_component(XmlComponent {
        id: normalize_casing("body"),
        renderer: Box::new(BodyRenderer::new()),
        inherit_vars: true,
    });
    map.register_component(XmlComponent {
        id: normalize_casing("div"),
        renderer: Box::new(DivRenderer::new()),
        inherit_vars: true,
    });
    map.register_component(XmlComponent {
        id: normalize_casing("p"),
        renderer: Box::new(TextRenderer::new()),
        inherit_vars: true,
    });
    
    // Register heading elements (h1-h6)
    map.register_component(XmlComponent {
        id: "h1".to_string(),
        renderer: Box::new(TextRenderer::with_css("h1", "font-size: 2em; font-weight: bold; margin: 0.67em 0;")),
        inherit_vars: true,
    });
    map.register_component(XmlComponent {
        id: "h2".to_string(),
        renderer: Box::new(TextRenderer::with_css("h2", "font-size: 1.5em; font-weight: bold; margin: 0.83em 0;")),
        inherit_vars: true,
    });
    map.register_component(XmlComponent {
        id: "h3".to_string(),
        renderer: Box::new(TextRenderer::with_css("h3", "font-size: 1.17em; font-weight: bold; margin: 1em 0;")),
        inherit_vars: true,
    });
    map.register_component(XmlComponent {
        id: "h4".to_string(),
        renderer: Box::new(TextRenderer::with_css("h4", "font-size: 1em; font-weight: bold; margin: 1.33em 0;")),
        inherit_vars: true,
    });
    map.register_component(XmlComponent {
        id: "h5".to_string(),
        renderer: Box::new(TextRenderer::with_css("h5", "font-size: 0.83em; font-weight: bold; margin: 1.67em 0;")),
        inherit_vars: true,
    });
    map.register_component(XmlComponent {
        id: "h6".to_string(),
        renderer: Box::new(TextRenderer::with_css("h6", "font-size: 0.67em; font-weight: bold; margin: 2.33em 0;")),
        inherit_vars: true,
    });
    
    // Register text formatting elements
    map.register_component(XmlComponent {
        id: "strong".to_string(),
        renderer: Box::new(StrongRenderer::new()),
        inherit_vars: true,
    });
    map.register_component(XmlComponent {
        id: "em".to_string(),
        renderer: Box::new(EmRenderer::new()),
        inherit_vars: true,
    });
    
    // Register hr element
    map.register_component(XmlComponent {
        id: "hr".to_string(),
        renderer: Box::new(HrRenderer::new()),
        inherit_vars: true,
    });
    
    // Register table elements
    map.register_component(XmlComponent {
        id: "table".to_string(),
        renderer: Box::new(TableRenderer::new()),
        inherit_vars: true,
    });
    map.register_component(XmlComponent {
        id: "tr".to_string(),
        renderer: Box::new(TrRenderer::new()),
        inherit_vars: true,
    });
    map.register_component(XmlComponent {
        id: "th".to_string(),
        renderer: Box::new(ThRenderer::new()),
        inherit_vars: true,
    });
    map.register_component(XmlComponent {
        id: "td".to_string(),
        renderer: Box::new(TdRenderer::new()),
        inherit_vars: true,
    });
    
    // Register list elements
    map.register_component(XmlComponent {
        id: "ul".to_string(),
        renderer: Box::new(UlRenderer::new()),
        inherit_vars: true,
    });
    map.register_component(XmlComponent {
        id: "ol".to_string(),
        renderer: Box::new(OlRenderer::new()),
        inherit_vars: true,
    });
    map.register_component(XmlComponent {
        id: "li".to_string(),
        renderer: Box::new(LiRenderer::new()),
        inherit_vars: true,
    });
    
    // Register img component
    map.register_component(XmlComponent {
        id: "img".to_string(),
        renderer: Box::new(ImgComponent {}),
        inherit_vars: false,
    });

    map
}