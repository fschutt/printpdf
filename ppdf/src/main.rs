//! ppdf: PDF <-> JSON <-> PDF on top of printpdf.
//!
//! Every command reads from a file or stdin ("-" or no argument) and writes to a
//! file or stdout, so it composes with pipes and `jq`:
//!
//! ```sh
//! ppdf json in.pdf | jq '.pages |= .[1:]' | ppdf pdf - -o out.pdf
//! ```
//!
//! Exit codes: 0 success, 1 error, 2 success but printpdf reported an
//! error-severity warning (something in the input was skipped).

mod binaries;
mod pages;

use clap::{Args, Parser, Subcommand, ValueEnum};
use printpdf::{
    Base64OrRaw, GeneratePdfOptions, ImageOptimizationOptions, OutputImageFormat, PdfDocument,
    PdfParseErrorSeverity, PdfParseOptions, PdfSaveOptions, PdfToSvgOptions, PdfWarnMsg,
    PreflightProfile, XObject, preflight_bytes,
};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::io::{IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// Top-level `$schema` of the JSON document. The major version changes only when
/// existing documents stop being readable.
const SCHEMA: &str = "printpdf-json/1";

type Res<T> = Result<T, Box<dyn std::error::Error>>;

#[derive(Parser)]
#[command(
    name = "ppdf",
    version,
    about = "PDF <-> JSON <-> PDF, text and box extraction, SVG rendering, HTML to PDF, preflight checks"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,

    /// How to print printpdf's warnings on stderr
    #[arg(long, global = true, value_enum, default_value_t = WarnFormat::Text)]
    warnings: WarnFormat,

    /// Include info-level messages in the text output (of the warnings, and of `check`)
    #[arg(short, long, global = true)]
    verbose: bool,

    /// Stop at the first parse error instead of skipping what cannot be read
    #[arg(long, global = true)]
    strict: bool,
}

#[derive(Clone, Copy, ValueEnum)]
enum WarnFormat {
    /// "warning[page 3, op 12]: message"
    Text,
    /// One JSON object per line
    Json,
    /// Print nothing
    Off,
}

#[derive(Subcommand)]
enum Cmd {
    /// PDF to the JSON document model
    Json(JsonArgs),
    /// JSON document model to PDF
    Pdf(PdfArgs),
    /// Plain text of the pages
    Text(TextArgs),
    /// Text lines, words and positions as JSON (hOCR-style)
    Boxes(BoxesArgs),
    /// One SVG per page
    Svg(SvgArgs),
    /// Pages, fonts, images, metadata and warnings as JSON
    Info(InfoArgs),
    /// List the fonts of a PDF, and write the font programs with -o
    Fonts(ExtractArgs),
    /// List the images of a PDF, and write them as files with -o
    Images(ExtractArgs),
    /// HTML to PDF
    Html(HtmlArgs),
    /// Check a PDF for problems (preflight). Exit code 0: no errors, 1: errors found, 2: unreadable
    Check(CheckArgs),
}

#[derive(Clone, Copy, ValueEnum)]
enum ProfileArg {
    /// Structure and resource integrity, no standard
    General,
    /// PDF/A-1b
    #[value(name = "pdfa-1b")]
    Pdfa1b,
    /// PDF/A-2b
    #[value(name = "pdfa-2b")]
    Pdfa2b,
    /// PDF/X-1a:2001
    #[value(name = "pdfx-1a")]
    Pdfx1a,
    /// PDF/X-3:2002
    #[value(name = "pdfx-3")]
    Pdfx3,
}

impl ProfileArg {
    fn profile(self) -> PreflightProfile {
        match self {
            ProfileArg::General => PreflightProfile::General,
            ProfileArg::Pdfa1b => PreflightProfile::PdfA1b,
            ProfileArg::Pdfa2b => PreflightProfile::PdfA2b,
            ProfileArg::Pdfx1a => PreflightProfile::PdfX1a_2001,
            ProfileArg::Pdfx3 => PreflightProfile::PdfX3_2002,
        }
    }
}

#[derive(Args)]
struct CheckArgs {
    /// Input PDF ("-" or none: stdin)
    input: Option<PathBuf>,
    /// The rules to apply
    #[arg(long, value_enum, default_value_t = ProfileArg::General)]
    profile: ProfileArg,
    /// Print the report as JSON
    #[arg(long)]
    json: bool,
}

#[derive(Args)]
struct JsonArgs {
    /// Input PDF ("-" or none: stdin)
    input: Option<PathBuf>,
    /// Output file (default: stdout)
    #[arg(short, long)]
    output: Option<PathBuf>,
    /// Keep only these pages, e.g. 1-3,7 (1-based, order is kept)
    #[arg(long)]
    pages: Option<String>,
    /// Write fonts and image pixels to DIR and leave {"$file": "..."} references
    #[arg(long, value_name = "DIR")]
    strip_binaries: Option<PathBuf>,
    /// Indent the JSON
    #[arg(long)]
    pretty: bool,
}

#[derive(Args)]
struct PdfArgs {
    /// Input JSON ("-" or none: stdin)
    input: Option<PathBuf>,
    /// Output file (default: stdout)
    #[arg(short, long)]
    output: Option<PathBuf>,
    /// Read the payloads that `ppdf json --strip-binaries` moved out of the document
    #[arg(long, value_name = "DIR")]
    binaries: Option<PathBuf>,
    /// Embed the complete fonts instead of subsets
    #[arg(long)]
    no_subset_fonts: bool,
    /// Skip stream compression and the pruning of unused objects
    #[arg(long)]
    no_optimize: bool,
    /// Re-encode images (lossy, quality 0.85). By default images are written as they are,
    /// so that a JSON round trip does not change a pixel.
    #[arg(long)]
    optimize_images: bool,
}

#[derive(Args)]
struct TextArgs {
    /// Input PDF ("-" or none: stdin)
    input: Option<PathBuf>,
    /// Output file (default: stdout)
    #[arg(short, long)]
    output: Option<PathBuf>,
    /// Only these pages, e.g. 1-3,7 (1-based)
    #[arg(long)]
    pages: Option<String>,
}

#[derive(Args)]
struct BoxesArgs {
    /// Input PDF ("-" or none: stdin)
    input: Option<PathBuf>,
    /// Output file (default: stdout)
    #[arg(short, long)]
    output: Option<PathBuf>,
    /// Only these pages, e.g. 1-3,7 (1-based)
    #[arg(long)]
    pages: Option<String>,
    /// Include the box of every glyph, not only of lines and words
    #[arg(long)]
    glyphs: bool,
    /// Indent the JSON
    #[arg(long)]
    pretty: bool,
}

#[derive(Args)]
struct SvgArgs {
    /// Input PDF ("-" or none: stdin)
    input: Option<PathBuf>,
    /// Directory for page-N.svg (default: stdout, which needs exactly one page)
    #[arg(short, long, value_name = "DIR")]
    output: Option<PathBuf>,
    /// Only these pages, e.g. 1-3,7 (1-based)
    #[arg(long)]
    pages: Option<String>,
}

#[derive(Args)]
struct InfoArgs {
    /// Input PDF ("-" or none: stdin)
    input: Option<PathBuf>,
}

#[derive(Args)]
struct ExtractArgs {
    /// Input PDF ("-" or none: stdin)
    input: Option<PathBuf>,
    /// Write the files into DIR (default: only list them)
    #[arg(short, long, value_name = "DIR")]
    output: Option<PathBuf>,
}

#[derive(Args)]
struct HtmlArgs {
    /// Input HTML ("-" or none: stdin)
    input: Option<PathBuf>,
    /// Output file (default: stdout)
    #[arg(short, long)]
    output: Option<PathBuf>,
    /// CSS file added to the document, after its own styles (repeatable)
    #[arg(long, value_name = "FILE")]
    css: Vec<PathBuf>,
    /// Page width in mm
    #[arg(long, default_value_t = 210.0)]
    width: f32,
    /// Page height in mm
    #[arg(long, default_value_t = 297.0)]
    height: f32,
    /// Margin on all sides in mm
    #[arg(long)]
    margin: Option<f32>,
    /// Directory of images for <img src="file.png"> (named by file name)
    #[arg(long, value_name = "DIR")]
    images: Option<PathBuf>,
    /// Directory of fonts for font-family (named by file stem: Roboto.ttf is "Roboto")
    #[arg(long, value_name = "DIR")]
    fonts: Option<PathBuf>,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    // `check` has its own exit codes: 0 no errors, 1 errors found, 2 unreadable.
    if let Cmd::Check(a) = &cli.cmd {
        return cmd_check(a, cli.verbose);
    }
    let mut warnings = Vec::new();
    let result = run(&cli, &mut warnings);
    report_warnings(&warnings, cli.warnings, cli.verbose);

    match result {
        Err(e) => {
            eprintln!("ppdf: error: {e}");
            ExitCode::from(1)
        }
        Ok(())
            if warnings
                .iter()
                .any(|w| w.severity == PdfParseErrorSeverity::Error) =>
        {
            ExitCode::from(2)
        }
        Ok(()) => ExitCode::SUCCESS,
    }
}

fn run(cli: &Cli, w: &mut Vec<PdfWarnMsg>) -> Res<()> {
    match &cli.cmd {
        Cmd::Json(a) => cmd_json(a, cli.strict, w),
        Cmd::Pdf(a) => cmd_pdf(a, w),
        Cmd::Text(a) => cmd_text(a, cli.strict, w),
        Cmd::Boxes(a) => cmd_boxes(a, cli.strict, w),
        Cmd::Svg(a) => cmd_svg(a, cli.strict, w),
        Cmd::Info(a) => cmd_info(a, cli.strict, w),
        Cmd::Fonts(a) => cmd_fonts(a, cli.strict, w),
        Cmd::Images(a) => cmd_images(a, cli.strict, w),
        Cmd::Html(a) => cmd_html(a, w),
        // Handled in `main`, because of its exit codes.
        Cmd::Check(_) => Ok(()),
    }
}

fn cmd_check(a: &CheckArgs, verbose: bool) -> ExitCode {
    let bytes = match read_input(&a.input) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("ppdf: error: {e}");
            return ExitCode::from(2);
        }
    };
    let profile = a.profile.profile();
    let report = preflight_bytes(&bytes, profile);
    let unreadable = report.findings.iter().any(|f| f.rule == "file-unreadable");

    if a.json {
        match serde_json::to_string_pretty(&report) {
            Ok(text) => println!("{text}"),
            Err(e) => {
                eprintln!("ppdf: error: cannot serialize the report: {e}");
                return ExitCode::from(2);
            }
        }
    } else {
        print_report(&report, verbose);
    }

    if unreadable {
        ExitCode::from(2)
    } else if report.has_errors() {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

fn print_report(report: &printpdf::preflight::PreflightReport, verbose: bool) {
    use printpdf::preflight::Severity;

    println!("Preflight, profile {}", report.profile.label());
    for f in &report.findings {
        if f.severity == Severity::Info && !verbose {
            continue;
        }
        let mut place = Vec::new();
        if let Some(p) = f.page {
            place.push(format!("page {p}"));
        }
        if let Some((n, g)) = f.object {
            place.push(format!("object {n} {g}"));
        }
        let place = if place.is_empty() {
            String::new()
        } else {
            format!(" ({})", place.join(", "))
        };
        let reference = f
            .standard_reference
            .map(|r| format!(" [{r}]"))
            .unwrap_or_default();
        println!(
            "{:<7} {}{}: {}{}",
            f.severity.as_str(),
            f.rule,
            place,
            f.message,
            reference
        );
    }

    let s = &report.summary;
    println!(
        "{} errors, {} warnings, {} infos ({} pages, {} objects visited)",
        s.errors, s.warnings, s.infos, s.pages_checked, s.objects_visited
    );
    if s.suppressed > 0 {
        println!("{} more findings were left out of the report", s.suppressed);
    }
    if s.infos > 0 && !verbose {
        println!("Information messages are hidden; add -v to show them.");
    }
    if report.has_errors() {
        return;
    }
    println!(
        "No errors found. This is not a certification of {}: the preflight checks only some rules.",
        report.profile.label()
    );
    if verbose {
        println!("Not checked:");
        for line in &report.not_checked {
            println!("  - {line}");
        }
    }
}

// ---------------------------------------------------------------- commands

fn cmd_json(a: &JsonArgs, strict: bool, w: &mut Vec<PdfWarnMsg>) -> Res<()> {
    let doc = load(&a.input, strict, w)?;
    let mut value =
        serde_json::to_value(&doc).map_err(|e| format!("cannot serialize the document: {e}"))?;

    if let Some(spec) = &a.pages {
        let selection = pages::parse(spec, doc.pages.len())?;
        if let Some(Value::Array(all)) = value.get_mut("pages") {
            *all = selection.iter().map(|&n| all[n - 1].clone()).collect();
        }
    }

    if let Some(dir) = &a.strip_binaries {
        binaries::strip(&mut value, dir)?;
    }

    // "$schema" goes first so the version is the first thing a reader sees.
    let mut top = Map::new();
    top.insert("$schema".to_string(), Value::String(SCHEMA.to_string()));
    if let Value::Object(map) = value {
        top.extend(map);
    }
    write_json(&a.output, &Value::Object(top), a.pretty)
}

fn cmd_pdf(a: &PdfArgs, w: &mut Vec<PdfWarnMsg>) -> Res<()> {
    let bytes = read_input(&a.input)?;
    let mut value: Value =
        serde_json::from_slice(&bytes).map_err(|e| format!("the input is not valid JSON: {e}"))?;
    check_schema(&mut value)?;
    if let Some(dir) = &a.binaries {
        binaries::inline(&mut value, dir)?;
    }

    let doc: PdfDocument = serde_json::from_value(value)
        .map_err(|e| format!("the input is not a printpdf document: {e}"))?;
    let opts = PdfSaveOptions {
        optimize: !a.no_optimize,
        subset_fonts: !a.no_subset_fonts,
        image_optimization: a.optimize_images.then(ImageOptimizationOptions::default),
        ..PdfSaveOptions::default()
    };
    let pdf = doc.save(&opts, w);
    write_output(&a.output, &pdf, true)
}

fn cmd_text(a: &TextArgs, strict: bool, w: &mut Vec<PdfWarnMsg>) -> Res<()> {
    let doc = load(&a.input, strict, w)?;
    let selection = select(&a.pages, doc.pages.len())?;
    let text = doc.extract_text();

    // Pages are separated by a form feed, like pdftotext does.
    let mut out = String::new();
    for (i, &n) in selection.iter().enumerate() {
        if i > 0 {
            out.push('\x0c');
        }
        for line in text.get(n - 1).into_iter().flatten() {
            out.push_str(line);
            out.push('\n');
        }
    }
    write_output(&a.output, out.as_bytes(), false)
}

fn cmd_boxes(a: &BoxesArgs, strict: bool, w: &mut Vec<PdfWarnMsg>) -> Res<()> {
    let doc = load(&a.input, strict, w)?;
    let selection = select(&a.pages, doc.pages.len())?;

    let mut out = Vec::new();
    for &n in &selection {
        let boxes = doc.pages[n - 1].extract_text_boxes(&doc.resources);
        let mut value = serde_json::to_value(&boxes)?;
        if let Value::Object(map) = &mut value {
            map.insert("page".to_string(), json!(n));
        }
        if !a.glyphs {
            remove_key(&mut value, "glyphs");
        }
        out.push(value);
    }
    write_json(&a.output, &Value::Array(out), a.pretty)
}

fn cmd_svg(a: &SvgArgs, strict: bool, w: &mut Vec<PdfWarnMsg>) -> Res<()> {
    let doc = load(&a.input, strict, w)?;
    let selection = select(&a.pages, doc.pages.len())?;

    let mut svgs = Vec::new();
    for &n in &selection {
        let svg = doc
            .page_to_svg(n, &PdfToSvgOptions::default(), w)
            .ok_or_else(|| format!("page {n} does not exist"))?;
        svgs.push((n, svg));
    }

    match a.output.as_deref() {
        Some(dir) if dir != Path::new("-") => {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
            let width = doc.pages.len().to_string().len();
            for (n, svg) in svgs {
                let path = dir.join(format!("page-{n:0width$}.svg"));
                std::fs::write(&path, svg).map_err(|e| format!("{}: {e}", path.display()))?;
            }
            Ok(())
        }
        _ if svgs.len() == 1 => write_output(&None, svgs[0].1.as_bytes(), false),
        _ => Err("more than one page selected: use -o DIR, or pick one page with --pages".into()),
    }
}

fn cmd_info(a: &InfoArgs, strict: bool, w: &mut Vec<PdfWarnMsg>) -> Res<()> {
    let doc = load(&a.input, strict, w)?;
    let meta = &doc.metadata.info;

    let pages: Vec<Value> = doc
        .pages
        .iter()
        .enumerate()
        .map(|(i, p)| {
            json!({
                "page": i + 1,
                "width_pt": p.media_box.width.0,
                "height_pt": p.media_box.height.0,
            })
        })
        .collect();

    let fonts: Vec<Value> = doc
        .resources
        .fonts
        .map
        .iter()
        .map(|(id, f)| {
            json!({
                "id": id.0,
                "name": f.parsed_font.font_name,
                "program_bytes": f.parsed_font.source_bytes().map(|b| b.len()),
            })
        })
        .collect();

    let images = doc
        .resources
        .xobjects
        .map
        .values()
        .filter(|x| matches!(x, XObject::Image(_)))
        .count();

    let count = |s: PdfParseErrorSeverity| w.iter().filter(|m| m.severity == s).count();
    let info = json!({
        "pdf_version": meta.version,
        "title": meta.document_title,
        "author": meta.author,
        "creator": meta.creator,
        "producer": meta.producer,
        "page_count": doc.pages.len(),
        "pages": pages,
        "fonts": fonts,
        "image_count": images,
        "warnings": {
            "error": count(PdfParseErrorSeverity::Error),
            "warning": count(PdfParseErrorSeverity::Warning),
            "info": count(PdfParseErrorSeverity::Info),
        },
    });
    write_json(&None, &info, true)
}

fn cmd_fonts(a: &ExtractArgs, strict: bool, w: &mut Vec<PdfWarnMsg>) -> Res<()> {
    let doc = load(&a.input, strict, w)?;
    if let Some(dir) = &a.output {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }

    let mut list = Vec::new();
    for (id, font) in &doc.resources.fonts.map {
        let bytes = font.parsed_font.source_bytes();
        let kind = bytes.as_ref().map(|b| font_kind(b.as_slice()));
        let mut entry = json!({
            "id": id.0,
            "name": font.parsed_font.font_name,
            "bytes": bytes.as_ref().map(|b| b.len()),
            "format": kind.map(|(name, _)| name),
        });

        if let (Some(dir), Some(bytes), Some((_, ext))) = (&a.output, &bytes, kind) {
            let file = format!("{}.{ext}", file_stem(&id.0));
            let path = dir.join(&file);
            std::fs::write(&path, bytes.as_slice())
                .map_err(|e| format!("{}: {e}", path.display()))?;
            entry["file"] = json!(file);
        }
        list.push(entry);
    }
    write_json(&None, &Value::Array(list), true)
}

fn cmd_images(a: &ExtractArgs, strict: bool, w: &mut Vec<PdfWarnMsg>) -> Res<()> {
    let doc = load(&a.input, strict, w)?;
    if let Some(dir) = &a.output {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }

    let mut list = Vec::new();
    for (id, xobject) in &doc.resources.xobjects.map {
        let XObject::Image(image) = xobject else {
            continue;
        };
        let mut entry = json!({
            "id": id.0,
            "width": image.width,
            "height": image.height,
            "format": format!("{:?}", image.data_format),
        });

        if let Some(dir) = &a.output {
            match image.encode_to_bytes(&[OutputImageFormat::Png, OutputImageFormat::Jpeg]) {
                Ok((bytes, format)) => {
                    let ext = match format {
                        OutputImageFormat::Jpeg => "jpg".to_string(),
                        other => format!("{other:?}").to_lowercase(),
                    };
                    let file = format!("{}.{ext}", file_stem(&id.0));
                    let path = dir.join(&file);
                    std::fs::write(&path, bytes).map_err(|e| format!("{}: {e}", path.display()))?;
                    entry["file"] = json!(file);
                }
                Err(e) => entry["error"] = json!(e),
            }
        }
        list.push(entry);
    }
    write_json(&None, &Value::Array(list), true)
}

fn cmd_html(a: &HtmlArgs, w: &mut Vec<PdfWarnMsg>) -> Res<()> {
    let bytes = read_input(&a.input)?;
    let mut html = String::from_utf8(bytes).map_err(|_| "the HTML is not valid UTF-8")?;

    if !a.css.is_empty() {
        let mut css = String::new();
        for path in &a.css {
            let text =
                std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
            css.push_str(&text);
            css.push('\n');
        }
        html = inject_css(&html, &css);
    }

    let images = match &a.images {
        Some(dir) => load_dir(dir, false)?,
        None => BTreeMap::new(),
    };
    let fonts = match &a.fonts {
        Some(dir) => load_dir(dir, true)?,
        None => BTreeMap::new(),
    };

    let options = GeneratePdfOptions {
        page_width: Some(a.width),
        page_height: Some(a.height),
        margin_top: a.margin,
        margin_right: a.margin,
        margin_bottom: a.margin,
        margin_left: a.margin,
        ..GeneratePdfOptions::default()
    };

    let doc = PdfDocument::from_html(&html, &images, &fonts, &options, w)?;
    let pdf = doc.save(&PdfSaveOptions::default(), w);
    write_output(&a.output, &pdf, true)
}

// ----------------------------------------------------------------- helpers

fn load(input: &Option<PathBuf>, strict: bool, w: &mut Vec<PdfWarnMsg>) -> Res<PdfDocument> {
    let bytes = read_input(input)?;
    let opts = PdfParseOptions {
        fail_on_error: strict,
    };
    Ok(PdfDocument::parse(&bytes, &opts, w)?)
}

fn select(spec: &Option<String>, count: usize) -> Res<Vec<usize>> {
    match spec {
        Some(spec) => Ok(pages::parse(spec, count)?),
        None => Ok((1..=count).collect()),
    }
}

/// Accepts documents without a `$schema` (written by other tools) and refuses a
/// major version this ppdf does not know.
fn check_schema(value: &mut Value) -> Res<()> {
    let Value::Object(map) = value else {
        return Ok(());
    };
    let Some(schema) = map.remove("$schema") else {
        return Ok(());
    };

    let known = schema
        .as_str()
        .and_then(|s| s.split_once('/'))
        .is_some_and(|(name, version)| {
            name == "printpdf-json" && version.split('.').next() == Some("1")
        });
    if known {
        Ok(())
    } else {
        Err(format!("unsupported $schema {schema}, this ppdf reads {SCHEMA}").into())
    }
}

fn remove_key(value: &mut Value, key: &str) {
    match value {
        Value::Object(map) => {
            map.remove(key);
            for (_, v) in map.iter_mut() {
                remove_key(v, key);
            }
        }
        Value::Array(items) => items.iter_mut().for_each(|v| remove_key(v, key)),
        _ => {}
    }
}

/// Puts `css` in a `<style>` element right after `<head>`, or in front of the
/// document when it has no head.
fn inject_css(html: &str, css: &str) -> String {
    let style = format!("<style>\n{css}</style>\n");
    let lower = html.to_ascii_lowercase();

    let mut from = 0;
    while let Some(i) = lower[from..].find("<head") {
        let start = from + i;
        let after = lower.as_bytes().get(start + 5).copied();
        // "<header>" also starts with "<head": only '>' or whitespace ends the tag name.
        if matches!(after, Some(b'>') | Some(b' ' | b'\t' | b'\n' | b'\r')) {
            if let Some(close) = lower[start..].find('>') {
                let at = start + close + 1;
                return format!("{}{}{}", &html[..at], style, &html[at..]);
            }
        }
        from = start + 5;
    }
    format!("{style}{html}")
}

fn load_dir(dir: &Path, by_stem: bool) -> Res<BTreeMap<String, Base64OrRaw>> {
    let mut map = BTreeMap::new();
    let entries = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for entry in entries {
        let path = entry?.path();
        if !path.is_file() {
            continue;
        }
        let name = if by_stem {
            path.file_stem()
        } else {
            path.file_name()
        };
        let Some(name) = name.and_then(|n| n.to_str()) else {
            continue;
        };
        let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        map.insert(name.to_string(), Base64OrRaw::Raw(bytes));
    }
    Ok(map)
}

/// Format name and file extension of a font program, from its first bytes.
fn font_kind(bytes: &[u8]) -> (&'static str, &'static str) {
    match bytes.get(..4) {
        Some(b"OTTO") => ("opentype-cff", "otf"),
        Some(b"ttcf") => ("truetype-collection", "ttc"),
        Some(b"wOFF") => ("woff", "woff"),
        Some(b"wOF2") => ("woff2", "woff2"),
        Some([0, 1, 0, 0]) | Some(b"true") => ("truetype", "ttf"),
        _ => ("unknown", "bin"),
    }
}

/// A resource id as a safe file name.
fn file_stem(id: &str) -> String {
    id.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn read_input(path: &Option<PathBuf>) -> Res<Vec<u8>> {
    match path.as_deref() {
        Some(p) if p != Path::new("-") => {
            Ok(std::fs::read(p).map_err(|e| format!("{}: {e}", p.display()))?)
        }
        _ => {
            let mut buf = Vec::new();
            std::io::stdin().lock().read_to_end(&mut buf)?;
            Ok(buf)
        }
    }
}

fn write_output(path: &Option<PathBuf>, bytes: &[u8], binary: bool) -> Res<()> {
    match path.as_deref() {
        Some(p) if p != Path::new("-") => {
            Ok(std::fs::write(p, bytes).map_err(|e| format!("{}: {e}", p.display()))?)
        }
        _ => {
            if binary && std::io::stdout().is_terminal() {
                return Err(
                    "refusing to write a PDF to the terminal, use -o FILE or a pipe".into(),
                );
            }
            let mut out = std::io::stdout().lock();
            match out.write_all(bytes).and_then(|()| out.flush()) {
                // `ppdf ... | head` closes the pipe early; that is not an error.
                Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => Ok(()),
                other => Ok(other?),
            }
        }
    }
}

fn write_json(path: &Option<PathBuf>, value: &Value, pretty: bool) -> Res<()> {
    let mut text = if pretty {
        serde_json::to_string_pretty(value)?
    } else {
        serde_json::to_string(value)?
    };
    text.push('\n');
    write_output(path, text.as_bytes(), false)
}

fn report_warnings(warnings: &[PdfWarnMsg], format: WarnFormat, verbose: bool) {
    match format {
        WarnFormat::Off => {}
        WarnFormat::Json => {
            for w in warnings {
                if let Ok(line) = serde_json::to_string(w) {
                    eprintln!("{line}");
                }
            }
        }
        WarnFormat::Text => {
            for w in warnings {
                let level = match w.severity {
                    PdfParseErrorSeverity::Error => "error",
                    PdfParseErrorSeverity::Warning => "warning",
                    PdfParseErrorSeverity::Info if verbose => "info",
                    PdfParseErrorSeverity::Info => continue,
                };
                eprintln!("{level}[page {}, op {}]: {}", w.page, w.op_id, w.msg);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn css_goes_after_head_but_not_after_header() {
        let html = "<html><body><header>x</header></body></html>";
        assert!(inject_css(html, "p{}").starts_with("<style>"));

        let html = "<html><head><title>t</title></head><body></body></html>";
        let out = inject_css(html, "p{}");
        assert!(out.starts_with("<html><head><style>"), "{out}");

        let html = "<HTML><HEAD lang=en></HEAD></HTML>";
        assert!(inject_css(html, "p{}").starts_with("<HTML><HEAD lang=en><style>"));
    }

    #[test]
    fn schema_check() {
        let mut ok = json!({"$schema": "printpdf-json/1", "pages": []});
        assert!(check_schema(&mut ok).is_ok());
        assert!(ok.get("$schema").is_none());

        let mut future = json!({"$schema": "printpdf-json/2"});
        assert!(check_schema(&mut future).is_err());

        let mut none = json!({"pages": []});
        assert!(check_schema(&mut none).is_ok());
    }

    #[test]
    fn font_programs_are_recognised() {
        assert_eq!(font_kind(b"OTTO\0\0").1, "otf");
        assert_eq!(font_kind(&[0, 1, 0, 0, 9]).1, "ttf");
        assert_eq!(font_kind(b"junk").1, "bin");
    }
}
