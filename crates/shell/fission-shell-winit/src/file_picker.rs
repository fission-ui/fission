use fission_core::{
    Bytes, CapabilityCtx, FissionDataStreamError, FissionDataStreamErrorKind, PickOpenFilesError,
    PickOpenFilesRequest, PickOpenFilesResult, PickedFile, PICK_OPEN_FILES,
};
use fission_shell::async_host::AsyncRegistry;
use futures_core::Stream;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::task::{Context, Poll};

const FILE_STREAM_CHUNK_SIZE: usize = 64 * 1024;

pub fn register_file_picker_capability(async_registry: &mut AsyncRegistry) {
    async_registry.register_operation_capability(
        PICK_OPEN_FILES,
        |request: PickOpenFilesRequest, ctx: CapabilityCtx| async move {
            let files = pick_open_files(request, &ctx)?;
            Ok::<_, PickOpenFilesError>(PickOpenFilesResult { files })
        },
    );
}

fn pick_open_files(
    request: PickOpenFilesRequest,
    ctx: &CapabilityCtx,
) -> Result<Vec<PickedFile>, PickOpenFilesError> {
    let mut dialog = rfd::FileDialog::new();
    let extensions = normalized_extensions(&request);
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
        .map(|path| picked_file_from_path(path, ctx))
        .collect()
}

fn picked_file_from_path(
    path: PathBuf,
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
    let stream = ctx.register_data_stream(Box::pin(FileDataStream::new(path)));

    Ok(PickedFile {
        name,
        content_type,
        byte_len: Some(metadata.len()),
        stream,
    })
}

fn normalized_extensions(request: &PickOpenFilesRequest) -> Vec<String> {
    let mut out = Vec::new();
    for extension in &request.extensions {
        let extension = extension.trim().trim_start_matches('.');
        if !extension.is_empty() {
            out.push(extension.to_string());
        }
    }
    for mime_type in &request.mime_types {
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
    file: Option<File>,
    done: bool,
}

impl FileDataStream {
    fn new(path: PathBuf) -> Self {
        Self {
            path,
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

        let mut buffer = vec![0; FILE_STREAM_CHUNK_SIZE];
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
    use fission_core::collect_data_stream;

    #[test]
    fn file_data_stream_reads_selected_file_in_chunks() {
        let path = std::env::temp_dir().join(format!("fission-file-stream-{}", std::process::id()));
        std::fs::write(&path, b"hello streamed file").unwrap();

        let bytes = pollster::block_on(collect_data_stream(Box::pin(FileDataStream::new(
            path.clone(),
        ))))
        .unwrap();

        let _ = std::fs::remove_file(path);
        assert_eq!(bytes.as_ref(), b"hello streamed file");
    }
}
