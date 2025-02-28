/// WASM / JS API, only enabled on WASM
#[cfg(target_family = "wasm")]
pub mod api;
/// WASM API Datastructures, can be used even on non-WASM targets
pub mod structs;
