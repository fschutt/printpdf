#!/usr/bin/env bash
# Runs the main examples. Each one writes its PDF (e.g. text_example.pdf) into the
# repo root, where *.pdf is git-ignored; tests/parse_roundtrip_corpus.rs uses
# the *_example.pdf files as a parsing corpus when they exist.
# Usage: bash scripts/gen-examples.sh
set -euo pipefail
cd "$(dirname "$0")/.."

cargo run --release --example advanced
cargo run --release --example bookmarks
cargo run --release --example graphics
cargo run --release --example html --features html
cargo run --release --example image --features png
cargo run --release --example layers
cargo run --release --example multipage
cargo run --release --example svg
cargo run --release --example text
