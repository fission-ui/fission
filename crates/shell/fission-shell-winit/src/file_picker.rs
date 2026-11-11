use fission_core::{
    Bytes, CapabilityCtx, FileSystemError, FileSystemLocation, FissionDataStreamError,
    FissionDataStreamErrorKind, PickOpenFilesError, PickOpenFilesRequest, PickOpenFilesResult,
    PickSaveFileRequest, PickSaveFileResult, PickedFile, PICK_OPEN_FILES, PICK_SAVE_FILE,
};
use fission_shell::async_host::AsyncRegistry;
use futures_core::Stream;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::task::{Context, Poll};

const DEFAULT_FILE_STREAM_CHUNK_SIZE: usize = 256 * 1024;
const MAX_FILE_STREAM_CHUNK_SIZE: usize = 16 * 1024 * 1024;

pub fn register_file_picker_capability(async_registry: &mut AsyncRegistry) {
    async_registry.register_operation_capability(
        PICK_OPEN_FILES,
        |request: PickOpenFilesRequest, ctx: CapabilityCtx| async move {
            let files = pick_open_files(request, &ctx)?;
            Ok::<_, PickOpenFilesError>(PickOpenFilesResult { files })
        },
    );
    async_registry.register_operation_capability(
        PICK_SAVE_FILE,
        |request: PickSaveFileRequest, _| async move {
            pick_save_file_with(request, show_native_save_dialog)
        },
    );
}

fn show_native_save_dialog(
    request: &PickSaveFileRequest,
) -> Result<Option<PathBuf>, FileSystemError> {
    let mut dialog = rfd::FileDialog::new();
    if !request.suggested_name.trim().is_empty() {
        dialog = dialog.set_file_name(request.suggested_name.trim());
    }
    let extensions = normalized_extensions(&request.extensions, &request.mime_types);
    if !extensions.is_empty() {
        dialog = dialog.add_filter("Allowed files", &extensions);
    }
    Ok(dialog.save_file())
}

fn pick_save_file_with(
    request: PickSaveFileRequest,
    select_destination: impl FnOnce(&PickSaveFileRequest) -> Result<Option<PathBuf>, FileSystemError>,
) -> Result<PickSaveFileResult, FileSystemError> {
    let location = select_destination(&request)?
        .map(|path| FileSystemLocation::path(path.to_string_lossy().into_owned()));
    Ok(PickSaveFileResult { location })
}

fn pick_open_files(
    request: PickOpenFilesRequest,
    ctx: &CapabilityCtx,
) -> Result<Vec<PickedFile>, PickOpenFilesError> {
    let chunk_size = request
        .preferred_chunk_bytes
        .filter(|bytes| *bytes > 0)
        .map(|bytes| bytes as usize)
        .unwrap_or(DEFAULT_FILE_STREAM_CHUNK_SIZE);
    let chunk_size = chunk_size.min(MAX_FILE_STREAM_CHUNK_SIZE);
    let mut dialog = rfd::FileDialog::new();
    let extensions = normalized_extensions(&request.extensions, &request.mime_types);
    if !extensions.is_empty() {
        dialog = dialog.add_filter("Allowed files", &extensions);
    }

    let paths = if request.allow_multiple {
        dialog.pick_files().unwrap_or_default()
    } else {
        dialog.pick_file().into_iter().collect()
    };

    paths
        .into_iter()
        .map(|path| picked_file_from_path(path, chunk_size, ctx))
        .collect()
}

fn picked_file_from_path(
    path: PathBuf,
    chunk_size: usize,
    ctx: &CapabilityCtx,
) -> Result<PickedFile, PickOpenFilesError> {
    let metadata = std::fs::metadata(&path).map_err(|error| {
        PickOpenFilesError::new(
            "metadata_failed",
            format!(
                "failed to inspect selected file `{}`: {error}",
                path.display()
            ),
        )
    })?;
    if !metadata.is_file() {
        return Err(PickOpenFilesError::new(
            "not_a_file",
            format!("selected path `{}` is not a regular file", path.display()),
        ));
    }

    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("selected-file")
        .to_string();
    let content_type = content_type_for_path(&path).map(str::to_string);
    let stream = ctx.register_data_stream(Box::pin(FileDataStream::new(path, chunk_size)));

    Ok(PickedFile {
        name,
        content_type,
        byte_len: Some(metadata.len()),
        stream,
    })
}

fn normalized_extensions(extensions: &[String], mime_types: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for extension in extensions {
        let extension = extension.trim().trim_start_matches('.');
        if !extension.is_empty() {
            out.push(extension.to_string());
        }
    }
    for mime_type in mime_types {
        out.extend(extensions_for_mime_type(mime_type).map(str::to_string));
    }
    out.sort();
    out.dedup();
    out
}

fn extensions_for_mime_type(mime_type: &str) -> impl Iterator<Item = &'static str> {
    match mime_type {
        "image/*" => ["png", "jpg", "jpeg", "gif", "webp", "bmp", "svg"].as_slice(),
        "audio/*" => ["mp3", "wav", "ogg", "flac", "m4a"].as_slice(),
        "video/*" => ["mp4", "mov", "webm", "mkv"].as_slice(),
        "text/*" => ["txt", "md", "csv", "json", "yaml", "yml", "toml"].as_slice(),
        "application/pdf" => ["pdf"].as_slice(),
        "application/json" => ["json"].as_slice(),
        "text/plain" => ["txt"].as_slice(),
        "text/csv" => ["csv"].as_slice(),
        "text/markdown" => ["md"].as_slice(),
        _ => [].as_slice(),
    }
    .iter()
    .copied()
}

fn content_type_for_path(path: &Path) -> Option<&'static str> {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("png") => Some("image/png"),
        Some("jpg") | Some("jpeg") => Some("image/jpeg"),
        Some("gif") => Some("image/gif"),
        Some("webp") => Some("image/webp"),
        Some("bmp") => Some("image/bmp"),
        Some("svg") => Some("image/svg+xml"),
        Some("pdf") => Some("application/pdf"),
        Some("json") => Some("application/json"),
        Some("txt") => Some("text/plain"),
        Some("csv") => Some("text/csv"),
        Some("md") => Some("text/markdown"),
        Some("yaml") | Some("yml") => Some("application/yaml"),
        Some("toml") => Some("application/toml"),
        Some("mp3") => Some("audio/mpeg"),
        Some("wav") => Some("audio/wav"),
        Some("ogg") => Some("audio/ogg"),
        Some("flac") => Some("audio/flac"),
        Some("m4a") => Some("audio/mp4"),
        Some("mp4") => Some("video/mp4"),
        Some("mov") => Some("video/quicktime"),
        Some("webm") => Some("video/webm"),
        _ => None,
    }
}

struct FileDataStream {
    path: PathBuf,
    chunk_size: usize,
    file: Option<File>,
    done: bool,
}

impl FileDataStream {
    fn new(path: PathBuf, chunk_size: usize) -> Self {
        Self {
            path,
            chunk_size,
            file: None,
            done: false,
        }
    }
}

impl Stream for FileDataStream {
    type Item = Result<Bytes, FissionDataStreamError>;

    fn poll_next(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.done {
            return Poll::Ready(None);
        }

        if self.file.is_none() {
            match File::open(&self.path) {
                Ok(file) => self.file = Some(file),
                Err(error) => {
                    self.done = true;
                    return Poll::Ready(Some(Err(FissionDataStreamError::new(
                        FissionDataStreamErrorKind::Io,
                        format!(
                            "failed to open selected file `{}`: {error}",
                            self.path.display()
                        ),
                    ))));
                }
            }
        }

        let mut buffer = vec![0; self.chunk_size];
        let file = self.file.as_mut().expect("file was opened above");
        match file.read(&mut buffer) {
            Ok(0) => {
                self.done = true;
                Poll::Ready(None)
            }
            Ok(read) => {
                buffer.truncate(read);
                Poll::Ready(Some(Ok(Bytes::from(buffer))))
            }
            Err(error) => {
                self.done = true;
                Poll::Ready(Some(Err(FissionDataStreamError::new(
                    FissionDataStreamErrorKind::Io,
                    format!(
                        "failed to read selected file `{}`: {error}",
                        self.path.display()
                    ),
                ))))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::task::{Wake, Waker};

    struct NoopWaker;

    impl Wake for NoopWaker {
        fn wake(self: Arc<Self>) {}
    }

    #[test]
    fn save_picker_returns_selected_path_and_passes_request_to_provider() {
        let request = PickSaveFileRequest {
            suggested_name: "report.pdf".into(),
            extensions: vec!["pdf".into()],
            ..Default::default()
        };
        let result = pick_save_file_with(request, |request| {
            assert_eq!(request.suggested_name, "report.pdf");
            assert_eq!(request.extensions, ["pdf"]);
            Ok(Some(PathBuf::from("/tmp/report.pdf")))
        })
        .unwrap();

        assert_eq!(
            result.location,
            Some(FileSystemLocation::path("/tmp/report.pdf"))
        );
    }

    #[test]
    fn save_picker_represents_cancellation_as_an_empty_location() {
        let result = pick_save_file_with(PickSaveFileRequest::default(), |_| Ok(None)).unwrap();
        assert_eq!(result.location, None);
    }

    #[test]
    fn save_picker_preserves_typed_unsupported_provider_errors() {
        let error = pick_save_file_with(PickSaveFileRequest::default(), |_| {
            Err(FileSystemError::unsupported("pick_save_file"))
        })
        .unwrap_err();

        assert_eq!(error.code, "unsupported");
        assert!(error.message.contains("pick_save_file"));
    }

    #[test]
    fn save_picker_filters_normalize_extensions_and_mime_types() {
        assert_eq!(
            normalized_extensions(
                &[".pdf".into(), " md ".into(), "".into()],
                &["text/csv".into(), "application/pdf".into()],
            ),
            vec!["csv", "md", "pdf"]
        );
    }

    #[test]
    fn file_data_stream_reads_selected_file_in_chunks() {
        let path = std::env::temp_dir().join(format!("fission-file-stream-{}", std::process::id()));
        std::fs::write(&path, b"hello streamed file").unwrap();

        let mut stream = Box::pin(FileDataStream::new(path.clone(), 4));
        let waker = Waker::from(Arc::new(NoopWaker));
        let mut context = Context::from_waker(&waker);
        let mut chunks = Vec::new();
        loop {
            match stream.as_mut().poll_next(&mut context) {
                Poll::Ready(Some(Ok(chunk))) => chunks.push(chunk),
                Poll::Ready(Some(Err(error))) => panic!("file stream failed: {error:?}"),
                Poll::Ready(None) => break,
                Poll::Pending => panic!("native file reads complete synchronously"),
            }
        }

        let _ = std::fs::remove_file(path);
        assert_eq!(
            chunks.iter().map(Bytes::len).collect::<Vec<_>>(),
            [4, 4, 4, 4, 3]
        );
        assert_eq!(
            chunks
                .iter()
                .flat_map(|chunk| chunk.iter().copied())
                .collect::<Vec<_>>(),
            b"hello streamed file",
            "chunking must preserve file contents"
        );
    }
}
