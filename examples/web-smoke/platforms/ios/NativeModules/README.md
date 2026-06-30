# iOS native modules

This Swift package is the app-owned integration point for native capability modules.

Fission generates `Package.swift` from `fission.toml` `[native]` module declarations. Capability
crates can provide Swift sources or package dependencies here without adding product-specific
logic to Fission itself.
