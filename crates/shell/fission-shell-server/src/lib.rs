//! Server-side web shell for Fission.
//!
//! The server shell adapts real Fission widget routes to HTTP responses. It
//! keeps server data fetching, mutations, and long-running work aligned with
//! Fission jobs, actions/reducers, and services rather than introducing a
//! second server component model.

mod app;
mod cache;
mod protocol;
mod render;
mod route;
mod serve;

pub use app::{FissionServerApp, ServerRenderContext};
pub use cache::{
    Cache, CacheEntry, CacheError, CacheKey, CacheLayerPolicy, CacheMetadata, CachePipeline,
    CacheScope, CacheTag, CacheValue, Freshness, InvalidationReport, MokaCache, MokaCacheOptions,
    RenderedPage, StoredJobResult,
};
pub use protocol::{
    AriaPoliteness, DomBatch, DomOp, MainToWorker, NavigateMode, NavigateRequest, ScrollBlock,
    WorkerBoot, WorkerDomEvent, WorkerError, WorkerLog, WorkerLogLevel, WorkerRequest,
    WorkerRequestKind, WorkerResize, WorkerResponse, WorkerToMain,
};
pub use render::{RenderedServerRoute, ServerRenderer, ServerRequest, ServerResponse};
pub use route::{
    ClientAppPolicy, ProgressiveWorker, RevalidationPolicy, ServerPrivatePolicy,
    ServerRenderPolicy, ServerResourcePolicy, WasmIsland, WebRoute, WebRouteMode,
};
pub use serve::{serve, ServeOptions};

pub fn run_from_cli(app: FissionServerApp) -> anyhow::Result<()> {
    let args = CliArgs::parse(std::env::args().skip(1))?;
    match args.command.as_str() {
        "check" => {
            let renderer = ServerRenderer::new(app);
            for route in renderer.routes() {
                renderer.render_route(&route.path)?;
                println!("{}  {}  {:?}", route.path, route.title, route.mode);
            }
            Ok(())
        }
        "routes" => {
            for route in app.routes() {
                println!("{}  {}  {:?}", route.path, route.title, route.mode);
            }
            Ok(())
        }
        "serve" => serve(
            ServerRenderer::new(app),
            ServeOptions {
                host: args.host,
                port: args.port,
            },
        ),
        other => {
            anyhow::bail!("unknown server command `{other}`; expected check, routes, or serve")
        }
    }
}

#[derive(Debug)]
struct CliArgs {
    command: String,
    host: String,
    port: u16,
}

impl CliArgs {
    fn parse<I>(args: I) -> anyhow::Result<Self>
    where
        I: IntoIterator<Item = String>,
    {
        let mut command = None;
        let mut host = "127.0.0.1".to_string();
        let mut port = 8124u16;
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--host" => {
                    host = args
                        .next()
                        .ok_or_else(|| anyhow::anyhow!("--host requires a value"))?
                }
                "--port" => {
                    port = args
                        .next()
                        .ok_or_else(|| anyhow::anyhow!("--port requires a value"))?
                        .parse()
                        .map_err(|_| anyhow::anyhow!("--port must be an integer"))?;
                }
                "--release" => {}
                value if value.starts_with('-') => anyhow::bail!("unknown server flag `{value}`"),
                value => command = Some(value.to_string()),
            }
        }
        Ok(Self {
            command: command.unwrap_or_else(|| "serve".to_string()),
            host,
            port,
        })
    }
}
