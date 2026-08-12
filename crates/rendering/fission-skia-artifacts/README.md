# fission-skia-artifacts

This crate is Fission's single selection, download, cache, and payload-verification
authority for prebuilt Skia and CanvasKit artifacts. It is used by
`fission-skia-sys` during native builds and by the Fission CLI when preparing an
interactive Web app.

The bundled lock contains only exact, production-qualified release artifacts.
If no matching entry exists, resolution fails closed. Local unqualified artifact
trees are accepted only through explicit caller-owned development overrides.
