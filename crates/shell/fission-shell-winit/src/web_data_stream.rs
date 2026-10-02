use fission_core::{Bytes, FissionDataStreamError};
use futures_core::Stream;
use std::collections::VecDeque;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

const STREAM_QUEUE_CAPACITY: usize = 2;

#[derive(Default)]
struct BrowserDataStreamState {
    chunks: VecDeque<Result<Bytes, FissionDataStreamError>>,
    consumer_waker: Option<Waker>,
    producer_waker: Option<Waker>,
    done: bool,
    cancelled: bool,
}

pub(crate) struct BrowserDataStream {
    state: Arc<Mutex<BrowserDataStreamState>>,
}

#[derive(Clone)]
pub(crate) struct BrowserDataStreamSink {
    state: Arc<Mutex<BrowserDataStreamState>>,
}

pub(crate) fn browser_data_stream() -> (BrowserDataStream, BrowserDataStreamSink) {
    let state = Arc::new(Mutex::new(BrowserDataStreamState::default()));
    (
        BrowserDataStream {
            state: state.clone(),
        },
        BrowserDataStreamSink { state },
    )
}

impl Stream for BrowserDataStream {
    type Item = Result<Bytes, FissionDataStreamError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut state = self.state.lock().unwrap();
        if let Some(chunk) = state.chunks.pop_front() {
            if let Some(waker) = state.producer_waker.take() {
                waker.wake();
            }
            return Poll::Ready(Some(chunk));
        }
        if state.done {
            return Poll::Ready(None);
        }
        state.consumer_waker = Some(cx.waker().clone());
        Poll::Pending
    }
}

impl Drop for BrowserDataStream {
    fn drop(&mut self) {
        let mut state = self.state.lock().unwrap();
        state.cancelled = true;
        if let Some(waker) = state.producer_waker.take() {
            waker.wake();
        }
    }
}

impl BrowserDataStreamSink {
    pub(crate) async fn push(&self, chunk: Result<Bytes, FissionDataStreamError>) -> bool {
        let mut chunk = Some(chunk);
        std::future::poll_fn(|cx| {
            let mut state = self.state.lock().unwrap();
            if state.cancelled {
                return Poll::Ready(false);
            }
            if state.chunks.len() >= STREAM_QUEUE_CAPACITY {
                state.producer_waker = Some(cx.waker().clone());
                return Poll::Pending;
            }
            state.chunks.push_back(chunk.take().unwrap());
            if let Some(waker) = state.consumer_waker.take() {
                waker.wake();
            }
            Poll::Ready(true)
        })
        .await
    }

    pub(crate) fn finish(&self) {
        let mut state = self.state.lock().unwrap();
        state.done = true;
        if let Some(waker) = state.consumer_waker.take() {
            waker.wake();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::task::{RawWaker, RawWakerVTable};

    fn noop_waker() -> Waker {
        unsafe fn clone(data: *const ()) -> RawWaker {
            RawWaker::new(data, &VTABLE)
        }
        unsafe fn wake(_data: *const ()) {}
        unsafe fn wake_by_ref(_data: *const ()) {}
        unsafe fn drop(_data: *const ()) {}

        static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, wake, wake_by_ref, drop);
        unsafe { Waker::from_raw(RawWaker::new(std::ptr::null(), &VTABLE)) }
    }

    #[test]
    fn completed_browser_stream_ends_without_a_payload() {
        let (mut stream, sink) = browser_data_stream();
        sink.finish();
        let waker = noop_waker();
        let mut cx = Context::from_waker(&waker);

        assert!(matches!(
            Pin::new(&mut stream).poll_next(&mut cx),
            Poll::Ready(None)
        ));
    }
}
