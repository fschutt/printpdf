//! Shrinks the pictures of a finished PDF: each one is encoded again and swapped in if it
//! comes out smaller. Pages, fonts and text stay as they are.
//!
//! ```sh
//! cargo run --release --example optimize_images --features png -- in.pdf out.pdf [--black-and-white]
//! ```
//!
//! `--black-and-white` dithers greyscale pictures (Floyd-Steinberg), which are then written with
//! one bit per pixel: for scans of engravings, woodcuts and other black-and-white prints.

use printpdf::{optimize_images, ImageOptimizationOptions};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let black_and_white = args.iter().any(|a| a == "--black-and-white");
    let files: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();
    let [input, output] = files[..] else {
        eprintln!("usage: optimize_images <in.pdf> <out.pdf> [--black-and-white]");
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
    let mut warnings = Vec::new();
    let smaller = optimize_images(&pdf, &options, &mut warnings).expect("optimizing the PDF");
    std::fs::write(output, &smaller).expect("writing the PDF");

    for warning in &warnings {
        println!("{}", warning.msg);
    }
    println!(
        "{input}: {:.1} MB, {output}: {:.1} MB ({} pictures encoded again, {:.1} s)",
        pdf.len() as f64 / 1e6,
        smaller.len() as f64 / 1e6,
        warnings.len(),
        start.elapsed().as_secs_f64()
    );
}
