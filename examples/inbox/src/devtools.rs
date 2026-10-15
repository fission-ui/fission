// Keep the changing application modules in the final JIT crate. Dependencies
// remain normally compiled and are supplied by the host bridge.
include!("lib.rs");

fission_devtools::devtools_main!(devtools);
