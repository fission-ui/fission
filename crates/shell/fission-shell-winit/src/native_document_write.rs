//! Claim one-shot input before a native document provider truncates its output.
use fission_core::{CapabilityCtx, FileSystemError, FileWriteSource};
use std::fs::File;
use std::io::Write;

pub(super) async fn write_source(
    source: FileWriteSource,
    ctx: &CapabilityCtx,
    open_destination: impl FnOnce() -> Result<File, FileSystemError>,
) -> Result<u64, FileSystemError> {
    let mut stream = match &source {
        FileWriteSource::Bytes(_) => None,
        FileWriteSource::Stream(id) => Some(
            ctx.open_data_stream(*id)
                .map_err(|error| FileSystemError::new("stream_open_failed", error.to_string()))?,
        ),
    };
    let mut destination = open_destination()?;
    let byte_len = match source {
        FileWriteSource::Bytes(bytes) => {
            destination.write_all(&bytes).map_err(io_error)?;
            bytes.len() as u64
        }
        FileWriteSource::Stream(_) => {
            let stream = stream
                .as_mut()
                .expect("stream was claimed before opening destination");
            let mut total = 0_u64;
            while let Some(chunk) = std::future::poll_fn(|cx| stream.as_mut().poll_next(cx)).await {
                let chunk = chunk.map_err(|error| {
                    FileSystemError::new("stream_read_failed", error.to_string())
                })?;
                destination.write_all(&chunk).map_err(io_error)?;
                total = total.saturating_add(chunk.len() as u64);
            }
            total
        }
    };
    destination.flush().map_err(io_error)?;
    Ok(byte_len)
}

fn io_error(error: std::io::Error) -> FileSystemError {
    FileSystemError::new("write_failed", error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use fission_core::{single_chunk_data_stream, Bytes, DataStreamId, DataStreamRegistry};
    use std::cell::Cell;
    use std::future::Future;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::task::{Context, Poll, Waker};

    fn path() -> std::path::PathBuf {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let target = std::env::var_os("CARGO_TARGET_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join("test-tmp");
        std::fs::create_dir_all(&target).unwrap();
        target.join(format!(
            "picked-document-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ))
    }

    fn ready<T>(future: impl Future<Output = T>) -> T {
        match Box::pin(future)
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
        {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("test source unexpectedly blocked"),
        }
    }

    #[test]
    fn invalid_and_already_consumed_streams_leave_existing_document_untouched() {
        let path = path();
        std::fs::write(&path, b"previous document").unwrap();
        let ctx = CapabilityCtx::new_runtime(1, DataStreamRegistry::default());
        let consumed =
            ctx.register_data_stream(single_chunk_data_stream(Bytes::from_static(b"new")));
        drop(ctx.open_data_stream(consumed).unwrap());
        for id in [DataStreamId(u64::MAX), consumed] {
            let opened = Cell::new(false);
            let result = ready(write_source(FileWriteSource::Stream(id), &ctx, || {
                opened.set(true);
                File::create(&path).map_err(io_error)
            }));
            assert_eq!(result.unwrap_err().code, "stream_open_failed");
            assert!(!opened.get());
            assert_eq!(std::fs::read(&path).unwrap(), b"previous document");
        }
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn valid_stream_is_consumed_once_and_reports_written_length() {
        let path = path();
        let ctx = CapabilityCtx::new_runtime(2, DataStreamRegistry::default());
        let id = ctx.register_data_stream(single_chunk_data_stream(Bytes::from_static(b"updated")));
        let written = ready(write_source(FileWriteSource::Stream(id), &ctx, || {
            File::create(&path).map_err(io_error)
        }))
        .unwrap();
        assert_eq!(written, 7);
        assert_eq!(std::fs::read(&path).unwrap(), b"updated");
        assert!(ctx.open_data_stream(id).is_err());
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn output_io_errors_preserve_the_existing_write_failed_code() {
        let path = path();
        std::fs::write(&path, b"previous document").unwrap();
        let ctx = CapabilityCtx::new_runtime(3, DataStreamRegistry::default());
        let error = ready(write_source(
            FileWriteSource::Bytes(b"updated".to_vec()),
            &ctx,
            || File::open(&path).map_err(io_error),
        ))
        .unwrap_err();
        assert_eq!(error.code, "write_failed");
        assert_eq!(std::fs::read(&path).unwrap(), b"previous document");
        std::fs::remove_file(path).unwrap();
    }
}
