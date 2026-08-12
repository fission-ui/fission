#[cfg(not(target_arch = "wasm32"))]
use std::sync::mpsc::{self, SyncSender, TrySendError};
#[cfg(not(target_arch = "wasm32"))]
use std::sync::{Arc, Mutex, OnceLock};

use fission_ir::op::{HttpHeader, ImageSource};

use super::AcquisitionKey;

pub(super) const MAX_SOURCE_BYTES: usize = 64 * 1024 * 1024;
const MAX_HEADER_BYTES: usize = 64 * 1024;
#[cfg(not(target_arch = "wasm32"))]
const NATIVE_WORK_QUEUE_CAPACITY: usize = 64;
#[cfg(not(target_arch = "wasm32"))]
const MAX_NATIVE_WORKERS: usize = 4;

pub(super) struct PendingLoad {
    pub(super) key: AcquisitionKey,
    pub(super) ticket: u64,
    pub(super) source: ImageSource,
}

pub(super) enum LoadOutcome {
    Ready(Vec<u8>),
    Failed(LoadFailure),
}

#[derive(Debug, Clone)]
pub(super) struct LoadFailure {
    pub(super) code: &'static str,
    pub(super) message: &'static str,
    pub(super) retryable: bool,
}

impl LoadFailure {
    const fn new(code: &'static str, message: &'static str, retryable: bool) -> Self {
        Self {
            code,
            message,
            retryable,
        }
    }

    pub(super) const fn too_large() -> Self {
        Self::new(
            "resource-source-too-large",
            "resource source exceeds the 64 MiB encoded-byte limit",
            false,
        )
    }
}

pub(super) type LoadCompletion = Box<dyn FnOnce(LoadOutcome) + Send + 'static>;

pub(super) trait SourceLoader: Send + Sync {
    fn start(&self, source: ImageSource, completion: LoadCompletion) -> Result<(), LoadFailure>;
}

pub(super) struct PlatformSourceLoader;

impl SourceLoader for PlatformSourceLoader {
    fn start(&self, source: ImageSource, completion: LoadCompletion) -> Result<(), LoadFailure> {
        start_platform_load(source, completion)
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn start_platform_load(source: ImageSource, completion: LoadCompletion) -> Result<(), LoadFailure> {
    native_worker_pool()?.submit(source, completion)
}

#[cfg(not(target_arch = "wasm32"))]
struct NativeWorkerPool {
    sender: SyncSender<NativeLoadJob>,
}

#[cfg(not(target_arch = "wasm32"))]
struct NativeLoadJob {
    source: ImageSource,
    completion: LoadCompletion,
}

#[cfg(not(target_arch = "wasm32"))]
impl NativeWorkerPool {
    fn new(worker_count: usize, queue_capacity: usize) -> Result<Self, LoadFailure> {
        let (sender, receiver) = mpsc::sync_channel::<NativeLoadJob>(queue_capacity);
        let receiver = Arc::new(Mutex::new(receiver));
        for index in 0..worker_count {
            let receiver = Arc::clone(&receiver);
            std::thread::Builder::new()
                .name(format!("fission-resource-{index}"))
                .spawn(move || loop {
                    let received = receiver
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .recv();
                    let Ok(job) = received else {
                        break;
                    };
                    (job.completion)(load_native(job.source));
                })
                .map_err(|_| {
                    LoadFailure::new(
                        "resource-worker-pool-unavailable",
                        "bounded resource worker pool could not be started",
                        true,
                    )
                })?;
        }
        Ok(Self { sender })
    }

    fn submit(&self, source: ImageSource, completion: LoadCompletion) -> Result<(), LoadFailure> {
        match self.sender.try_send(NativeLoadJob { source, completion }) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_)) => Err(LoadFailure::new(
                "resource-worker-queue-saturated",
                "bounded resource worker queue is saturated",
                true,
            )),
            Err(TrySendError::Disconnected(_)) => Err(LoadFailure::new(
                "resource-worker-pool-unavailable",
                "bounded resource worker pool is unavailable",
                true,
            )),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn native_worker_pool() -> Result<&'static NativeWorkerPool, LoadFailure> {
    static POOL: OnceLock<Result<NativeWorkerPool, LoadFailure>> = OnceLock::new();
    match POOL.get_or_init(|| {
        let worker_count = std::thread::available_parallelism()
            .map(|count| count.get())
            .unwrap_or(1)
            .clamp(1, MAX_NATIVE_WORKERS);
        NativeWorkerPool::new(worker_count, NATIVE_WORK_QUEUE_CAPACITY)
    }) {
        Ok(pool) => Ok(pool),
        Err(failure) => Err(failure.clone()),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn load_native(source: ImageSource) -> LoadOutcome {
    match source {
        ImageSource::Asset { path } => read_file(&path, true),
        ImageSource::File { path } => read_file(&path, false),
        ImageSource::Network { url, headers, .. } => fetch_network(&url, &headers),
        ImageSource::Memory { bytes, .. } => bounded_bytes(bytes),
        ImageSource::SvgText { content } => bounded_bytes(content.into_bytes()),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn read_file(path: &str, asset: bool) -> LoadOutcome {
    use std::fs::File;
    use std::io::Read;

    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(_) => {
            return LoadOutcome::Failed(if asset {
                LoadFailure::new(
                    "resource-asset-open-failed",
                    "asset source could not be opened",
                    true,
                )
            } else {
                LoadFailure::new(
                    "resource-file-open-failed",
                    "file source could not be opened",
                    true,
                )
            });
        }
    };
    if file
        .metadata()
        .ok()
        .is_some_and(|metadata| metadata.len() > MAX_SOURCE_BYTES as u64)
    {
        return LoadOutcome::Failed(LoadFailure::too_large());
    }

    let mut bytes = Vec::new();
    match file
        .by_ref()
        .take(MAX_SOURCE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
    {
        Ok(_) => bounded_bytes(bytes),
        Err(_) => LoadOutcome::Failed(if asset {
            LoadFailure::new(
                "resource-asset-read-failed",
                "asset source could not be read",
                true,
            )
        } else {
            LoadFailure::new(
                "resource-file-read-failed",
                "file source could not be read",
                true,
            )
        }),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn fetch_network(url: &str, headers: &[HttpHeader]) -> LoadOutcome {
    use std::io::Read;

    if let Err(failure) = validate_headers(headers) {
        return LoadOutcome::Failed(failure);
    }

    let mut request = ureq::get(url).set("User-Agent", "FissionResourceLoader/0.10");
    for header in headers {
        request = request.set(&header.name, &header.value);
    }
    let response = match request.call() {
        Ok(response) => response,
        Err(ureq::Error::Status(status, _)) => {
            return LoadOutcome::Failed(LoadFailure::new(
                "resource-network-http-failed",
                if status == 408 || status == 429 || status >= 500 {
                    "network source returned a retryable HTTP status"
                } else {
                    "network source returned an unsuccessful HTTP status"
                },
                status == 408 || status == 429 || status >= 500,
            ));
        }
        Err(ureq::Error::Transport(_)) => {
            return LoadOutcome::Failed(LoadFailure::new(
                "resource-network-transport-failed",
                "network source transport failed",
                true,
            ));
        }
    };
    if content_length_exceeds(response.header("Content-Length")) {
        return LoadOutcome::Failed(LoadFailure::too_large());
    }

    let mut bytes = Vec::new();
    match response
        .into_reader()
        .take(MAX_SOURCE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
    {
        Ok(_) => bounded_bytes(bytes),
        Err(_) => LoadOutcome::Failed(LoadFailure::new(
            "resource-network-read-failed",
            "network source body could not be read",
            true,
        )),
    }
}

#[cfg(target_arch = "wasm32")]
fn start_platform_load(source: ImageSource, completion: LoadCompletion) -> Result<(), LoadFailure> {
    wasm_bindgen_futures::spawn_local(async move {
        completion(load_web(source).await);
    });
    Ok(())
}

#[cfg(target_arch = "wasm32")]
async fn load_web(source: ImageSource) -> LoadOutcome {
    match source {
        ImageSource::Asset { path } => fetch_web(&path, &[]).await,
        ImageSource::Network { url, headers, .. } => fetch_web(&url, &headers).await,
        ImageSource::File { .. } => LoadOutcome::Failed(LoadFailure::new(
            "resource-file-unsupported",
            "browser targets cannot open local file image sources",
            false,
        )),
        ImageSource::Memory { bytes, .. } => bounded_bytes(bytes),
        ImageSource::SvgText { content } => bounded_bytes(content.into_bytes()),
    }
}

#[cfg(target_arch = "wasm32")]
async fn fetch_web(url: &str, headers: &[HttpHeader]) -> LoadOutcome {
    use wasm_bindgen::JsCast;

    if let Err(failure) = validate_headers(headers) {
        return LoadOutcome::Failed(failure);
    }
    let Some(window) = web_sys::window() else {
        return LoadOutcome::Failed(LoadFailure::new(
            "resource-browser-window-unavailable",
            "browser window is unavailable for resource acquisition",
            true,
        ));
    };
    let init = web_sys::RequestInit::new();
    init.set_method("GET");
    init.set_mode(web_sys::RequestMode::Cors);
    let request = match web_sys::Request::new_with_str_and_init(url, &init) {
        Ok(request) => request,
        Err(_) => {
            return LoadOutcome::Failed(LoadFailure::new(
                "resource-network-request-invalid",
                "network source request could not be constructed",
                false,
            ));
        }
    };
    for header in headers {
        if request.headers().set(&header.name, &header.value).is_err() {
            return LoadOutcome::Failed(LoadFailure::new(
                "resource-network-header-invalid",
                "network source contains an invalid request header",
                false,
            ));
        }
    }
    let response =
        match wasm_bindgen_futures::JsFuture::from(window.fetch_with_request(&request)).await {
            Ok(response) => match response.dyn_into::<web_sys::Response>() {
                Ok(response) => response,
                Err(_) => {
                    return LoadOutcome::Failed(LoadFailure::new(
                        "resource-network-response-invalid",
                        "network source returned an invalid browser response",
                        true,
                    ));
                }
            },
            Err(_) => {
                return LoadOutcome::Failed(LoadFailure::new(
                    "resource-network-transport-failed",
                    "network source transport failed",
                    true,
                ));
            }
        };
    if !response.ok() {
        let status = response.status();
        return LoadOutcome::Failed(LoadFailure::new(
            "resource-network-http-failed",
            if status == 408 || status == 429 || status >= 500 {
                "network source returned a retryable HTTP status"
            } else {
                "network source returned an unsuccessful HTTP status"
            },
            status == 408 || status == 429 || status >= 500,
        ));
    }
    let content_length = response.headers().get("Content-Length").ok().flatten();
    if content_length_exceeds(content_length.as_deref()) {
        return LoadOutcome::Failed(LoadFailure::too_large());
    }
    let buffer = match response.array_buffer() {
        Ok(buffer) => match wasm_bindgen_futures::JsFuture::from(buffer).await {
            Ok(buffer) => buffer,
            Err(_) => {
                return LoadOutcome::Failed(LoadFailure::new(
                    "resource-network-read-failed",
                    "network source body could not be read",
                    true,
                ));
            }
        },
        Err(_) => {
            return LoadOutcome::Failed(LoadFailure::new(
                "resource-network-read-failed",
                "network source body could not be read",
                true,
            ));
        }
    };
    let encoded = js_sys::Uint8Array::new(&buffer);
    if encoded.length() as usize > MAX_SOURCE_BYTES {
        return LoadOutcome::Failed(LoadFailure::too_large());
    }
    let mut bytes = vec![0; encoded.length() as usize];
    encoded.copy_to(&mut bytes);
    LoadOutcome::Ready(bytes)
}

fn bounded_bytes(bytes: Vec<u8>) -> LoadOutcome {
    bounded_bytes_to(bytes, MAX_SOURCE_BYTES)
}

fn bounded_bytes_to(bytes: Vec<u8>, limit: usize) -> LoadOutcome {
    if bytes.len() > limit {
        LoadOutcome::Failed(LoadFailure::too_large())
    } else {
        LoadOutcome::Ready(bytes)
    }
}

fn content_length_exceeds(value: Option<&str>) -> bool {
    value
        .and_then(|value| value.trim().parse::<u64>().ok())
        .is_some_and(|length| length > MAX_SOURCE_BYTES as u64)
}

fn validate_headers(headers: &[HttpHeader]) -> Result<(), LoadFailure> {
    let total_bytes = headers.iter().fold(0_usize, |total, header| {
        total
            .saturating_add(header.name.len())
            .saturating_add(header.value.len())
    });
    if total_bytes > MAX_HEADER_BYTES {
        return Err(LoadFailure::new(
            "resource-network-headers-too-large",
            "network source request headers exceed the 64 KiB limit",
            false,
        ));
    }
    if headers.iter().any(|header| {
        header.name.is_empty()
            || !header.name.bytes().all(is_header_name_byte)
            || !header.value.bytes().all(is_header_value_byte)
    }) {
        return Err(LoadFailure::new(
            "resource-network-header-invalid",
            "network source contains an invalid request header",
            false,
        ));
    }
    Ok(())
}

fn is_header_name_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric()
        || matches!(
            byte,
            b'!' | b'#'
                | b'$'
                | b'%'
                | b'&'
                | b'\''
                | b'*'
                | b'+'
                | b'-'
                | b'.'
                | b'^'
                | b'_'
                | b'`'
                | b'|'
                | b'~'
        )
}

fn is_header_value_byte(byte: u8) -> bool {
    byte == b'\t' || (byte >= b' ' && byte != 0x7f)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_length_and_header_validation_fail_closed() {
        assert!(content_length_exceeds(Some("67108865")));
        assert!(!content_length_exceeds(Some("67108864")));
        assert!(!content_length_exceeds(Some("unknown")));

        assert!(validate_headers(&[HttpHeader {
            name: "Authorization".into(),
            value: "Bearer opaque".into(),
        }])
        .is_ok());
        let failure = validate_headers(&[HttpHeader {
            name: "X-Bad\r\nInjected".into(),
            value: "value".into(),
        }])
        .unwrap_err();
        assert_eq!(failure.code, "resource-network-header-invalid");
        assert!(!failure.retryable);
    }

    #[test]
    fn bounded_bytes_rejects_over_limit_without_publishing_payload() {
        assert!(matches!(bounded_bytes(vec![0; 8]), LoadOutcome::Ready(_)));
        assert!(matches!(
            bounded_bytes_to(vec![0; 9], 8),
            LoadOutcome::Failed(LoadFailure {
                code: "resource-source-too-large",
                ..
            })
        ));
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn native_file_loader_returns_encoded_source_bytes() {
        use std::sync::atomic::{AtomicU64, Ordering};

        static NEXT_FILE: AtomicU64 = AtomicU64::new(1);
        let nonce = NEXT_FILE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "fission-resource-loader-{}-{nonce}.bin",
            std::process::id()
        ));
        std::fs::write(&path, [1_u8, 2, 3, 4]).unwrap();

        let outcome = read_file(path.to_str().unwrap(), false);

        std::fs::remove_file(&path).unwrap();
        assert!(matches!(outcome, LoadOutcome::Ready(bytes) if bytes == [1, 2, 3, 4]));
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn bounded_native_queue_rejects_excess_work_explicitly() {
        use std::time::Duration;

        let pool = NativeWorkerPool::new(1, 1).unwrap();
        let source = ImageSource::Memory {
            bytes: vec![1],
            mime_type: None,
        };
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        pool.submit(
            source.clone(),
            Box::new(move |_| {
                started_tx.send(()).unwrap();
                release_rx.recv().unwrap();
            }),
        )
        .unwrap();
        started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        pool.submit(source.clone(), Box::new(|_| {})).unwrap();

        let failure = pool.submit(source, Box::new(|_| {})).unwrap_err();

        assert_eq!(failure.code, "resource-worker-queue-saturated");
        assert!(failure.retryable);
        release_tx.send(()).unwrap();
    }
}
