//! Shrinks a finished PDF: its pictures are encoded again (each one swapped in if it comes out
//! smaller), and the /ActualText that its glyphs read as anyway is dropped. Pages, fonts and
//! what the text reads as stay as they are.
//!
//! ```sh
//! cargo run --release --example optimize_pdf --features png -- in.pdf out.pdf [--black-and-white]
//! ```
//!
//! `--black-and-white` dithers greyscale pictures (Floyd-Steinberg), which are then written with
//! one bit per pixel: for scans of engravings, woodcuts and other black-and-white prints.

use printpdf::{optimize_images, optimize_text, ImageOptimizationOptions};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let black_and_white = args.iter().any(|a| a == "--black-and-white");
    let files: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();
    let [input, output] = files[..] else {
        eprintln!("usage: optimize_pdf <in.pdf> <out.pdf> [--black-and-white]");
        std::process::exit(2);
    };

    let pdf = std::fs::read(input).expect("reading the PDF");
    let options = ImageOptimizationOptions {
        dither_greyscale: Some(black_and_white),
        // the same pixels in fewer bytes: no picture is scaled down
        max_image_size: None,
        ..Default::default()
    };
    let start = std::time::Instant::now();
    let mut pictures = Vec::new();
    let smaller = optimize_images(&pdf, &options, &mut pictures).expect("encoding the pictures");
    let mut text = Vec::new();
    let smaller = optimize_text(&smaller, &mut text).expect("dropping /ActualText");
    std::fs::write(output, &smaller).expect("writing the PDF");

    println!("{} pictures encoded again", pictures.len());
    for message in &text {
        println!("{}", message.msg);
    }
    println!(
        "{input}: {:.1} MB, {output}: {:.1} MB ({:.1} s)",
        pdf.len() as f64 / 1e6,
        smaller.len() as f64 / 1e6,
        start.elapsed().as_secs_f64()
    );
}
