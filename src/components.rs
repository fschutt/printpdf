//! printpdf's custom XML/HTML components for the data-driven azul component system.
//!
//! As of azul-core 0.0.8 the old `XmlComponentTrait` was replaced by a data-driven
//! [`ComponentDef`] model (id = `collection:name`, a typed `data_model`, and bare
//! `render_fn` / `compile_fn` function pointers). The `builtin` library shipped with
//! azul-core already renders ~90 standard HTML tags (html/body/div/h1-h6/p/span/table/
//! tr/td/a/ul/li/…), so printpdf no longer needs to register those itself.
//!
//! This module therefore only defines printpdf's **custom** components — the ones that
//! are *not* part of azul's builtin library:
//!
//! * [`img_component_def`] — `<img>` (PDF image embedding). `img` is not a builtin tag.
//! * [`dynamic_xml_component_def`] — `<component name="…" args="…">` user-defined
//!   reusable components (printpdf issue #268).
//!
//! [`printpdf_default_components`] returns azul's builtin [`ComponentMap`] with a small
//! `"printpdf"` library (holding the two components above) appended.
//!
//! ## Runtime note
//!
//! The published azul-core `str_to_dom` rendering path (`render_dom_from_body_node_fast`
//! → `xml_node_to_fast_dom`) maps tags straight to `NodeType` via `tag_to_node_type` and
//! does **not** invoke any `ComponentDef::render_fn`. The `render_fn`/`compile_fn`
//! machinery is only consumed by the code-generation / preview path (`str_to_rust_code`).
//! These custom defs are therefore primarily descriptive for that path; see the crate
//! notes / issue #268 for the image-embedding follow-up.

use azul_core::{
    dom::{Dom, NodeType},
    styled_dom::StyledDom,
    xml::{
        ComponentDataField, ComponentDataModel, ComponentDef, ComponentFieldType, ComponentId,
        ComponentLibrary, ComponentMap, ComponentSource, ComponentDefaultValue,
        CompileTarget, OptionComponentDefaultValue, RenderDomError,
        ResultStringCompileError, ResultStyledDomRenderDomError,
    },
};
use azul_css::{css::Css, AzString};

/// Build a [`ComponentDataField`] with a rich type and optional default value.
///
/// Mirrors azul-core's private `data_field` helper. `required` is derived from
/// whether a default was supplied.
fn data_field(
    name: &str,
    ft: ComponentFieldType,
    default: Option<ComponentDefaultValue>,
    description: &str,
) -> ComponentDataField {
    let required = default.is_none();
    ComponentDataField {
        name: AzString::from(name),
        field_type: ft,
        default_value: match default {
            Some(d) => OptionComponentDefaultValue::Some(d),
            None => OptionComponentDefaultValue::None,
        },
        required,
        description: AzString::from(description),
    }
}

// ============================================================================
// <img> — PDF image embedding
// ============================================================================

/// Render function for the `<img>` component.
///
/// The new `render_fn` signature only receives the typed `ComponentDataModel`
/// (carrying string attributes such as `src` / `alt`); it has no access to the
/// decoded image bytes (those live in `XmlRenderOptions::images` on the printpdf
/// side, which is not threaded into azul's renderer). It therefore renders a
/// placeholder `<div>` carrying the same shape the old `ImgComponent` produced.
/// Actual image embedding is handled by printpdf when it walks the resulting
/// display list (see issue #268).
fn img_render_fn(
    _def: &ComponentDef,
    _data: &ComponentDataModel,
    _component_map: &ComponentMap,
) -> ResultStyledDomRenderDomError {
    let mut dom = Dom::create_node(NodeType::Div);
    let r: Result<StyledDom, RenderDomError> = Ok(StyledDom::create(&mut dom, Css::empty()));
    r.into()
}

/// Compile function for the `<img>` component (code-generation / preview path).
fn img_compile_fn(
    _def: &ComponentDef,
    target: &CompileTarget,
    _data: &ComponentDataModel,
    _indent: usize,
) -> ResultStringCompileError {
    let code = match target {
        CompileTarget::Rust => "Dom::create_node(NodeType::Div) /* img */",
        CompileTarget::C => "AzDom_createDiv() /* img */",
        CompileTarget::Cpp => "Dom::div() /* img */",
        CompileTarget::Python => "Dom.div() # img",
    };
    let r: Result<AzString, _> = Ok(AzString::from(code));
    r.into()
}

/// `printpdf:img` — the `<img>` element (PDF image embedding).
///
/// Mirrors azul-core's private `builtin_component_def`: a small typed data model
/// (`src`, `alt`) plus the printpdf-specific render / compile functions.
pub fn img_component_def() -> ComponentDef {
    ComponentDef {
        id: ComponentId::new("printpdf", "img"),
        display_name: AzString::from("Image"),
        description: AzString::from("HTML <img> element (embeds an image into the PDF)"),
        css: AzString::from(""),
        source: ComponentSource::Compiled,
        data_model: ComponentDataModel {
            name: AzString::from("ImgData"),
            description: AzString::from("Data model for <img>"),
            fields: vec![
                data_field(
                    "src",
                    ComponentFieldType::String,
                    None,
                    "Image source (URL or embedded image id)",
                ),
                data_field(
                    "alt",
                    ComponentFieldType::String,
                    Some(ComponentDefaultValue::String(AzString::from(""))),
                    "Alternate text",
                ),
            ]
            .into(),
        },
        render_fn: img_render_fn,
        compile_fn: img_compile_fn,
        render_fn_source: None.into(),
        compile_fn_source: None.into(),
    }
}

// ============================================================================
// <component> — user-defined dynamic XML components (#268)
// ============================================================================

/// Render function for the `<component>` element.
///
/// In the old API a `DynamicXmlComponent` captured the component's root XML
/// template (`self.root`) and instantiated it with substituted arguments on
/// render. The new `render_fn` is a bare `fn` pointer that cannot close over a
/// captured XML subtree, and the `ComponentDataModel` only carries scalar/typed
/// field values — not an arbitrary XML template. A faithful template-instantiating
/// port is therefore not expressible in this signature; we render an empty `<div>`
/// placeholder. See issue #268 for the follow-up.
fn dynamic_xml_render_fn(
    _def: &ComponentDef,
    _data: &ComponentDataModel,
    _component_map: &ComponentMap,
) -> ResultStyledDomRenderDomError {
    let mut dom = Dom::create_node(NodeType::Div);
    let r: Result<StyledDom, RenderDomError> = Ok(StyledDom::create(&mut dom, Css::empty()));
    r.into()
}

/// Compile function for the `<component>` element (code-generation / preview path).
fn dynamic_xml_compile_fn(
    _def: &ComponentDef,
    target: &CompileTarget,
    _data: &ComponentDataModel,
    _indent: usize,
) -> ResultStringCompileError {
    let code = match target {
        CompileTarget::Rust => "Dom::create_node(NodeType::Div) /* component */",
        CompileTarget::C => "AzDom_createDiv() /* component */",
        CompileTarget::Cpp => "Dom::div() /* component */",
        CompileTarget::Python => "Dom.div() # component",
    };
    let r: Result<AzString, _> = Ok(AzString::from(code));
    r.into()
}

/// `printpdf:component` — user-defined reusable component (`<component name="…" />`).
///
/// Port of the old `DynamicXmlComponent` (#268) to the data-driven model. The
/// `name` field identifies the component instance; the `args` field carries its
/// raw argument string (parsed downstream).
pub fn dynamic_xml_component_def() -> ComponentDef {
    ComponentDef {
        id: ComponentId::new("printpdf", "component"),
        display_name: AzString::from("Component"),
        description: AzString::from("User-defined reusable XML component (<component name=\"…\" />)"),
        css: AzString::from(""),
        source: ComponentSource::UserDefined,
        data_model: ComponentDataModel {
            name: AzString::from("ComponentData"),
            description: AzString::from("Data model for a user-defined <component>"),
            fields: vec![
                data_field(
                    "name",
                    ComponentFieldType::String,
                    None,
                    "Name of the component, e.g. \"test\" for <component name=\"test\" />",
                ),
                data_field(
                    "args",
                    ComponentFieldType::String,
                    Some(ComponentDefaultValue::String(AzString::from(""))),
                    "Raw component argument string, e.g. \"a: String\"",
                ),
            ]
            .into(),
        },
        render_fn: dynamic_xml_render_fn,
        compile_fn: dynamic_xml_compile_fn,
        render_fn_source: None.into(),
        compile_fn_source: None.into(),
    }
}

/// The printpdf custom component library: the components that are *not* part of
/// azul's builtin library (`img`, `component`).
pub fn printpdf_component_library() -> ComponentLibrary {
    ComponentLibrary {
        name: AzString::from("printpdf"),
        version: AzString::from("1.0.0"),
        description: AzString::from("printpdf custom components (img, component)"),
        exportable: false,
        modifiable: false,
        data_models: Vec::new().into(),
        enum_models: Vec::new().into(),
        components: vec![img_component_def(), dynamic_xml_component_def()].into(),
    }
}

/// Returns a [`ComponentMap`] with azul's builtin HTML element components plus
/// printpdf's custom components (`img`, `component`) registered.
///
/// Used by `src/html/mod.rs` `str_to_dom(...)`.
pub fn printpdf_default_components() -> ComponentMap {
    // Start from azul's builtin library (renders ~90 standard HTML tags) and
    // append the printpdf custom library.
    let mut map = ComponentMap::with_builtin();
    let mut libs = std::mem::replace(&mut map.libraries, Vec::new().into()).into_library_owned_vec();
    libs.push(printpdf_component_library());
    map.libraries = libs.into();
    map
}
