use crate::{hyper_adapter, ServerRenderer};
use anyhow::{Context, Result};
use hyper::service::{make_service_fn, service_fn};
use hyper::Server;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Eq)]
/// Network address used by the built-in SSR development server.
pub struct ServeOptions {
    /// Host name or IP address on which to listen.
    pub host: String,
    /// TCP port on which to listen.
    pub port: u16,
}

impl Default for ServeOptions {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 8124,
        }
    }
}

/// Runs the built-in Hyper server until it stops or encounters an error.
///
/// Production deployments may instead use one of the framework adapters and
/// integrate [`ServerRenderer`] into an existing HTTP server.
pub fn serve(renderer: ServerRenderer, options: ServeOptions) -> Result<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_io()
        .enable_time()
        .build()
        .context("failed to start server runtime")?;
    runtime.block_on(serve_async(renderer, options))
}

async fn serve_async(renderer: ServerRenderer, options: ServeOptions) -> Result<()> {
    let address: SocketAddr = format!("{}:{}", options.host, options.port)
        .parse()
        .with_context(|| {
            format!(
                "failed to parse server address {}:{}",
                options.host, options.port
            )
        })?;
    let renderer = Arc::new(renderer);
    let service = make_service_fn(move |_| {
        let renderer = renderer.clone();
        async move {
            Ok::<_, Infallible>(service_fn(move |request| {
                hyper_adapter::handle(renderer.clone(), request)
            }))
        }
    });

    println!("Serving Fission server app at http://{address}/");
    println!("Press Ctrl+C to stop.");
    Server::bind(&address)
        .serve(service)
        .await
        .context("server failed")?;
    Ok(())
}
