//! Server-side web shell for Fission.
//!
//! The server shell adapts real Fission widget routes to HTTP responses. It
//! keeps server data fetching, mutations, and long-running work aligned with
//! Fission jobs, actions/reducers, and services rather than introducing a
//! second server component model.

mod action_token;
mod adapters;
mod app;
mod artifacts;
mod cache;
mod config;
mod jobs;
mod protocol;
mod render;
mod route;
mod serve;

pub use action_token::{ServerActionSigner, SignedServerAction, VerifiedServerAction};
#[cfg(feature = "actix-adapter")]
pub use adapters::actix_adapter;
#[cfg(feature = "axum-adapter")]
pub use adapters::axum_adapter;
pub use adapters::hyper_adapter;
pub use app::{
    FissionServerApp, ServerEnvContext, ServerHttpContext, ServerRenderContext, StaticMount,
};
pub use artifacts::{
    BrowserArtifactBuild, BrowserArtifactBuildOptions, BrowserArtifactKind, BrowserArtifactPlan,
};
#[cfg(feature = "redis")]
pub use cache::RedisCache;
pub use cache::{
    Cache, CacheEntry, CacheError, CacheKey, CacheLayerPolicy, CacheMetadata, CachePipeline,
    CacheScope, CacheTag, CacheValue, Freshness, InvalidationReport, MokaCache, MokaCacheOptions,
    RenderedPage, StoredJobResult,
};
pub use config::{
    ServerBrowserArtifactConfig, ServerCacheConfig, ServerCacheLayerConfig, ServerCacheProvider,
    ServerHttpConfig, ServerIslandConfig, ServerIslandPreload, ServerRuntimeConfig, ServerSameSite,
    ServerSessionConfig, ServerSessionProvider, ServerWorkerBridge,
};
pub use jobs::{ServerJobCtx, ServerJobError, ServerJobRegistry};
pub use protocol::{
    AriaPoliteness, BrowserBridgeOutput, BrowserEventBinding, BrowserEventKind, DomBatch, DomOp,
    MainToWorker, NavigateMode, NavigateRequest, ScrollBlock, WorkerBoot, WorkerDomEvent,
    WorkerDomPolicy, WorkerError, WorkerLog, WorkerLogLevel, WorkerProtocolError, WorkerRequest,
    WorkerRequestKind, WorkerResize, WorkerResponse, WorkerToMain,
};
pub use render::{
    RenderedServerRoute, ServerRenderer, ServerRequest, ServerResponse, ServerSession,
    MAX_SERVER_ACTION_BODY_BYTES,
};
pub use route::{
    ClientAppPolicy, ProgressiveWorker, RevalidationPolicy, ServerPrivatePolicy,
    ServerRenderPolicy, ServerResourcePolicy, WasmIsland, WebRoute, WebRouteMode,
};
pub use serve::{serve, ServeOptions};

pub fn run_from_cli(app: FissionServerApp) -> anyhow::Result<()> {
    let args = CliArgs::parse(std::env::args().skip(1))?;
    match args.command.as_str() {
        "check" => {
            let renderer = ServerRenderer::configured(app)?;
            for route in renderer.routes() {
                renderer.render_route(&route.path)?;
                println!("{}  {}  {:?}", route.path, route.title, route.mode);
            }
            Ok(())
        }
        "routes" => {
            let renderer = ServerRenderer::configured(app)?;
            for route in renderer.routes() {
                println!("{}  {}  {:?}", route.path, route.title, route.mode);
            }
            Ok(())
        }
        "serve" => serve(
            ServerRenderer::configured(app)?,
            ServeOptions {
                host: args.host,
                port: args.port,
            },
        ),
        "artifacts" => {
            let project_dir = app.project_dir.clone();
            let config = ServerRuntimeConfig::load(&project_dir)?;
            if !config.workers.separate_artifacts {
                anyhow::bail!(
                    "[server.workers].separate_artifacts = false is not supported; server workers are compiled as route-local artifacts"
                );
            }
            if !config.islands.separate_artifacts {
                anyhow::bail!(
                    "[server.islands].separate_artifacts = false is not supported; server islands are compiled as route-local artifacts"
                );
            }
            let output_dir = args
                .output_dir
                .unwrap_or_else(|| project_dir.join("target/fission/server"));
            let package_name = args
                .package_name
                .ok_or_else(|| anyhow::anyhow!("server artifacts requires --package-name"))?;
            let options = BrowserArtifactBuildOptions {
                project_dir,
                output_dir,
                package_name,
                package_default_features: args.package_default_features,
                package_features: args.package_features,
                release: args.release,
                compile: args.compile_artifacts,
            };
            let build = BrowserArtifactBuild::from_app(&app, &options)?;
            build.write_shims(&options)?;
            for plan in build.plans {
                println!("{}  {:?}  {}", plan.id, plan.kind, plan.artifact);
            }
            Ok(())
        }
        other => {
            anyhow::bail!(
                "unknown server command `{other}`; expected check, routes, serve, or artifacts"
            )
        }
    }
}

#[derive(Debug)]
struct CliArgs {
    command: String,
    host: String,
    port: u16,
    release: bool,
    package_name: Option<String>,
    package_default_features: bool,
    package_features: Vec<String>,
    output_dir: Option<std::path::PathBuf>,
    compile_artifacts: bool,
}

impl CliArgs {
    fn parse<I>(args: I) -> anyhow::Result<Self>
    where
        I: IntoIterator<Item = String>,
    {
        let mut command = None;
        let mut host = "127.0.0.1".to_string();
        let mut port = 8124u16;
        let mut release = false;
        let mut package_name = None;
        let mut package_default_features = true;
        let mut package_features = Vec::new();
        let mut output_dir = None;
        let mut compile_artifacts = true;
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
                "--package-name" => {
                    package_name = Some(
                        args.next()
                            .ok_or_else(|| anyhow::anyhow!("--package-name requires a value"))?,
                    )
                }
                "--package-no-default-features" => package_default_features = false,
                "--package-feature" => {
                    package_features
                        .push(args.next().ok_or_else(|| {
                            anyhow::anyhow!("--package-feature requires a value")
                        })?);
                }
                "--output-dir" => {
                    output_dir =
                        Some(std::path::PathBuf::from(args.next().ok_or_else(|| {
                            anyhow::anyhow!("--output-dir requires a value")
                        })?))
                }
                "--no-compile" => compile_artifacts = false,
                "--release" => release = true,
                value if value.starts_with('-') => anyhow::bail!("unknown server flag `{value}`"),
                value => command = Some(value.to_string()),
            }
        }
        Ok(Self {
            command: command.unwrap_or_else(|| "serve".to_string()),
            host,
            port,
            release,
            package_name,
            package_default_features,
            package_features,
            output_dir,
            compile_artifacts,
        })
    }
}
