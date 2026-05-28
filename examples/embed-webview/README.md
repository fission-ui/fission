# WebView Embed

WebView Embed demonstrates a bounded host-backed web surface. The example uses a deterministic `data:` URL so it can render without a network connection while still exercising the same widget path a remote URL would use.

Use this example when you want to understand how web content is embedded as part of a Fission app instead of opening a separate browser window.

## Run it

```bash
cargo run -p embed-webview
```

## What to look at

- [`src/lib.rs`](src/lib.rs) defines `WEBVIEW_DEMO_URL`, `WebViewEmbedState`, and `WebViewEmbedApp`.
- [`src/main.rs`](src/main.rs) runs the library app through the desktop shell.
- The `WebView` construction in [`src/lib.rs`](src/lib.rs) shows id, URL, optional user agent, width, and height.

## Features exercised

- `WebView` widget embedding.
- Host-backed surface sizing and layout reservation.
- Deterministic local demo content through a `data:` URL.
- Themed container chrome around embedded content.

## Learning path

Read the `WEBVIEW_DEMO_URL` constant first so you know what content is expected, then follow it into the `WebView` widget. For production apps, replace the data URL with a controlled local page or remote URL and keep the same sizing/container pattern.
