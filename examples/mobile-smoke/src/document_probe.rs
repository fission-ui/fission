//! Device-facing checks using the real host providers, never replacement providers.
use std::collections::HashMap;

use fission::prelude::*;

use crate::SmokeState;

pub const INITIAL_NOTE: &str =
    "Fission document QA\nRevision 1: saved through the native picker.\n";
pub const UPDATED_NOTE: &str =
    "Fission document QA\nRevision 2: updated through the retained save handle.\n";
const MAX_IMPORT_BYTES: usize = 64 * 1024;
const PRINT_DOCUMENT: &[u8] = include_bytes!("../fixtures/two-page-print.pdf");

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Phase {
    #[default]
    Ready,
    PickingSave,
    Writing,
    PickingOpen,
    Reading,
    Printing,
    Saved,
    Imported,
    Cancelled,
    PrintSubmitted,
    PrintPresented,
    Failed,
}

impl Phase {
    fn key(self) -> &'static str {
        match self {
            Self::Ready => "documents.status.ready",
            Self::PickingSave => "documents.status.picking_save",
            Self::Writing => "documents.status.writing",
            Self::PickingOpen => "documents.status.picking_open",
            Self::Reading => "documents.status.reading",
            Self::Printing => "documents.status.printing",
            Self::Saved => "documents.status.saved",
            Self::Imported => "documents.status.imported",
            Self::Cancelled => "documents.status.cancelled",
            Self::PrintSubmitted => "documents.status.print_submitted",
            Self::PrintPresented => "documents.status.print_presented",
            Self::Failed => "documents.status.failed",
        }
    }

    fn pending(self) -> bool {
        matches!(
            self,
            Self::PickingSave | Self::Writing | Self::PickingOpen | Self::Reading | Self::Printing
        )
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct DocumentState {
    phase: Phase,
    destination: Option<SaveFileDestination>,
    file_name: Option<String>,
    imported_text: Option<String>,
    byte_len: usize,
    error: Option<String>,
}

impl DocumentState {
    fn begin(&mut self, phase: Phase) {
        self.phase = phase;
        self.error = None;
    }

    fn fail(&mut self, error: impl Into<String>) {
        self.phase = Phase::Failed;
        self.error = Some(error.into());
    }
}

pub fn create_env() -> Env {
    let mut env = Env::default();
    env.locale = Locale::from("en-US");
    for (locale, source) in [
        ("en-US", include_str!("../i18n/en-US.properties")),
        ("es-ES", include_str!("../i18n/es-ES.properties")),
    ] {
        let messages = translation_messages(source);
        env.i18n.add_bundle(TranslationBundle {
            locale: Locale::from(locale),
            messages,
        });
    }
    env
}

fn translation_messages(source: &str) -> HashMap<String, String> {
    source
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let (key, value) = line
                .split_once('=')
                .expect("translations use one key=value per line");
            (key.to_owned(), value.to_owned())
        })
        .collect()
}

pub struct ReadDocument;

impl JobSpec for ReadDocument {
    type Request = DataStreamId;
    type Ok = String;
    type Err = String;
    const NAME: &'static str = "mobile-smoke.read-document";
}

pub const READ_DOCUMENT: JobRef<ReadDocument> = JobRef::new("mobile-smoke.read-document");

pub async fn read_document(stream: DataStreamId, ctx: JobCtx) -> Result<String, String> {
    let mut stream = ctx
        .open_data_stream(stream)
        .map_err(|error| error.to_string())?;
    let mut bytes = Vec::new();
    while let Some(chunk) = std::future::poll_fn(|cx| stream.as_mut().poll_next(cx)).await {
        let chunk = chunk.map_err(|error| error.to_string())?;
        if chunk.len() > MAX_IMPORT_BYTES.saturating_sub(bytes.len()) {
            return Err("documents.error.import_limit".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    String::from_utf8(bytes).map_err(|_| "documents.error.invalid_utf8".into())
}

#[fission_reducer(ChooseSaveDocument)]
fn choose_save_document(state: &mut SmokeState, cx: &mut ReducerContext<SmokeState>) {
    if state.documents.phase.pending() {
        return;
    }
    state.documents.begin(Phase::PickingSave);
    let ok = cx
        .effects
        .bind(SaveDocumentChosen, reduce!(save_document_chosen));
    let err = cx.effects.bind(DocumentFailed, reduce!(document_failed));
    cx.effects
        .capability(
            PICK_SAVE_FILE,
            PickSaveFileRequest {
                suggested_name: "fission-document-qa.txt".into(),
                extensions: vec!["txt".into()],
                mime_types: vec!["text/plain".into()],
                ..Default::default()
            },
        )
        .on_ok(ok)
        .on_err(err);
}

#[fission_reducer(SaveDocumentChosen)]
fn save_document_chosen(state: &mut SmokeState, cx: &mut ReducerContext<SmokeState>) {
    let Some(result) = cx.input.capability_ok(PICK_SAVE_FILE) else {
        state.documents.fail("documents.error.missing_result");
        return;
    };
    let Some(destination) = result.destination else {
        state.documents.phase = Phase::Cancelled;
        return;
    };
    state.documents.destination = Some(destination);
    write_note(state, cx, INITIAL_NOTE);
}

#[fission_reducer(UpdateSavedDocument)]
fn update_saved_document(state: &mut SmokeState, cx: &mut ReducerContext<SmokeState>) {
    if !state.documents.phase.pending() {
        write_note(state, cx, UPDATED_NOTE);
    }
}

fn write_note(state: &mut SmokeState, cx: &mut ReducerContext<SmokeState>, text: &str) {
    let Some(destination) = state.documents.destination.clone() else {
        return;
    };
    state.documents.begin(Phase::Writing);
    let ok = cx.effects.bind(DocumentWritten, reduce!(document_written));
    let err = cx.effects.bind(DocumentFailed, reduce!(document_failed));
    let source = FileWriteSource::Bytes(text.as_bytes().to_vec());
    match destination {
        SaveFileDestination::WritableFile(file) => {
            state.documents.file_name = Some(file.name.clone());
            cx.effects
                .capability(WRITE_PICKED_FILE, WritePickedFileRequest { file, source })
                .on_ok(ok)
                .on_err(err);
        }
        SaveFileDestination::Location(location) => {
            // Desktop hosts may return a path. Without an enabled filesystem
            // provider this reports an explicit unsupported-capability error.
            state.documents.file_name = Some(location.file_system_path().as_str().into());
            cx.effects
                .capability(
                    WRITE_FILE,
                    WriteFileRequest {
                        location,
                        source,
                        create_parents: false,
                        overwrite: true,
                    },
                )
                .on_ok(ok)
                .on_err(err);
        }
    }
}

#[fission_reducer(DocumentWritten)]
fn document_written(state: &mut SmokeState, cx: &mut ReducerContext<SmokeState>) {
    let byte_len = cx
        .input
        .capability_ok(WRITE_PICKED_FILE)
        .map(|result| result.byte_len)
        .or_else(|| {
            cx.input
                .capability_ok(WRITE_FILE)
                .map(|result| result.byte_len)
        });
    if let Some(byte_len) = byte_len {
        state.documents.byte_len = byte_len as usize;
        state.documents.phase = Phase::Saved;
    } else {
        state.documents.fail("documents.error.missing_result");
    }
}

#[fission_reducer(ChooseImportDocument)]
fn choose_import_document(state: &mut SmokeState, cx: &mut ReducerContext<SmokeState>) {
    if state.documents.phase.pending() {
        return;
    }
    state.documents.begin(Phase::PickingOpen);
    let ok = cx
        .effects
        .bind(ImportDocumentChosen, reduce!(import_document_chosen));
    let err = cx.effects.bind(DocumentFailed, reduce!(document_failed));
    cx.effects
        .capability(
            PICK_OPEN_FILES,
            PickOpenFilesRequest {
                allow_multiple: false,
                extensions: vec!["txt".into()],
                mime_types: vec!["text/plain".into()],
                preferred_chunk_bytes: Some(4096),
            },
        )
        .on_ok(ok)
        .on_err(err);
}

#[fission_reducer(ImportDocumentChosen)]
fn import_document_chosen(state: &mut SmokeState, cx: &mut ReducerContext<SmokeState>) {
    let Some(result) = cx.input.capability_ok(PICK_OPEN_FILES) else {
        state.documents.fail("documents.error.missing_result");
        return;
    };
    let Some(file) = result.files.into_iter().next() else {
        state.documents.phase = Phase::Cancelled;
        return;
    };
    state.documents.file_name = Some(file.name);
    state.documents.begin(Phase::Reading);
    let ok = cx.effects.bind(DocumentRead, reduce!(document_read));
    let err = cx.effects.bind(DocumentFailed, reduce!(document_failed));
    cx.effects
        .app(READ_DOCUMENT, file.stream)
        .on_ok(ok)
        .on_err(err);
}

#[fission_reducer(DocumentRead)]
fn document_read(state: &mut SmokeState, cx: &mut ReducerContext<SmokeState>) {
    if let Some(text) = cx.input.job_ok(READ_DOCUMENT) {
        state.documents.byte_len = text.len();
        state.documents.imported_text = Some(text);
        state.documents.phase = Phase::Imported;
    } else {
        state.documents.fail("documents.error.missing_result");
    }
}

#[fission_reducer(PrintDocument)]
fn print_document(state: &mut SmokeState, cx: &mut ReducerContext<SmokeState>) {
    if state.documents.phase.pending() {
        return;
    }
    state.documents.begin(Phase::Printing);
    let ok = cx
        .effects
        .bind(DocumentPrintFinished, reduce!(document_print_finished));
    let err = cx.effects.bind(DocumentFailed, reduce!(document_failed));
    cx.effects
        .print_pdf(PrintPdfRequest {
            pdf: FileWriteSource::Bytes(PRINT_DOCUMENT.to_vec()),
            job_name: Some("Fission two-page print QA".into()),
            ..Default::default()
        })
        .on_ok(ok)
        .on_err(err);
}

#[fission_reducer(DocumentPrintFinished)]
fn document_print_finished(state: &mut SmokeState, cx: &mut ReducerContext<SmokeState>) {
    if let Some(result) = cx.input.capability_ok(PRINT_PDF) {
        state.documents.phase = match result.outcome {
            PrintOutcome::Presented => Phase::PrintPresented,
            PrintOutcome::Submitted => Phase::PrintSubmitted,
            PrintOutcome::Cancelled => Phase::Cancelled,
        };
    } else {
        state.documents.fail("documents.error.missing_result");
    }
}

#[fission_reducer(DocumentFailed)]
fn document_failed(state: &mut SmokeState, cx: &mut ReducerContext<SmokeState>) {
    let error = cx
        .input
        .capability_error(PICK_SAVE_FILE)
        .map(|error| error.message)
        .or_else(|| {
            cx.input
                .capability_error(WRITE_PICKED_FILE)
                .map(|error| error.message)
        })
        .or_else(|| {
            cx.input
                .capability_error(WRITE_FILE)
                .map(|error| error.message)
        })
        .or_else(|| {
            cx.input
                .capability_error(PICK_OPEN_FILES)
                .map(|error| error.message)
        })
        .or_else(|| {
            cx.input
                .capability_error(PRINT_PDF)
                .map(|error| error.message)
        })
        .or_else(|| cx.input.job_err(READ_DOCUMENT))
        .or_else(|| {
            cx.input
                .capability_error_message(PICK_SAVE_FILE)
                .map(str::to_owned)
        })
        .or_else(|| {
            cx.input
                .capability_error_message(WRITE_PICKED_FILE)
                .map(str::to_owned)
        })
        .or_else(|| {
            cx.input
                .capability_error_message(WRITE_FILE)
                .map(str::to_owned)
        })
        .or_else(|| {
            cx.input
                .capability_error_message(PICK_OPEN_FILES)
                .map(str::to_owned)
        })
        .or_else(|| {
            cx.input
                .capability_error_message(PRINT_PDF)
                .map(str::to_owned)
        })
        .unwrap_or_else(|| "documents.error.missing_result".into());
    state.documents.fail(error);
}

fission_component! {
    pub DocumentProbe<SmokeState>
    view(_probe, state, cx) {
        let env = cx.view.env();
        let tokens = &env.theme.tokens;
        let pending = state.documents.phase.pending();
        let error = state.documents.error.as_ref().map(|message| {
            env.i18n.get(&env.locale, message).unwrap_or(message).to_owned()
        });
    } => Column(
        gap: tokens.spacing.m,
        children: [
            Text(TextContent::Key("documents.title".into())).size(tokens.typography.font_size_xl),
            Text(TextContent::Key("documents.instructions".into())),
            Button(
                child: Text(TextContent::Key("documents.save".into())),
                key: "save",
                disabled: pending,
                on_press: with_reducer!(cx.build, ChooseSaveDocument, choose_save_document),
            ).semantics_identifier("mobile-smoke.documents.save"),
            Button(
                child: Text(TextContent::Key("documents.import".into())),
                key: "import",
                disabled: pending,
                on_press: with_reducer!(cx.build, ChooseImportDocument, choose_import_document),
            ).semantics_identifier("mobile-smoke.documents.import"),
            Button(
                child: Text(TextContent::Key("documents.update".into())),
                key: "update",
                disabled: pending || state.documents.destination.is_none(),
                on_press: with_reducer!(cx.build, UpdateSavedDocument, update_saved_document),
            ).semantics_identifier("mobile-smoke.documents.update"),
            Button(
                child: Text(TextContent::Key("documents.print".into())),
                key: "print",
                disabled: pending,
                on_press: with_reducer!(cx.build, PrintDocument, print_document),
            ).semantics_identifier("mobile-smoke.documents.print"),
            if pending { Spinner() },
            Text(TextContent::Key(state.documents.phase.key().into()))
                .semantics_identifier("mobile-smoke.documents.status"),
            if let Some(error) = error {
                Text(error).color(tokens.colors.text_primary)
                    .semantics_identifier("mobile-smoke.documents.error")
            },
            if let Some(name) = &state.documents.file_name {
                Column(gap: tokens.spacing.s, children: [
                    Text(TextContent::Key("documents.file".into())),
                    Text(name.clone()),
                ])
            },
            Row(gap: tokens.spacing.s, children: [
                Text(TextContent::Key("documents.bytes".into())),
                Text(state.documents.byte_len.to_string()),
            ]),
            if let Some(text) = &state.documents.imported_text {
                Column(gap: tokens.spacing.s, children: [
                    Text(TextContent::Key("documents.imported_text".into())),
                    Text(text.clone()).semantics_identifier("mobile-smoke.documents.content"),
                ])
            },
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::future::Future;
    use std::task::{Context, Poll, Waker};

    fn ready<F: Future>(future: F) -> F::Output {
        let mut future = std::pin::pin!(future);
        match future
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
        {
            Poll::Ready(result) => result,
            Poll::Pending => panic!("single-chunk fixture must be immediately ready"),
        }
    }

    fn read_fixture(bytes: Vec<u8>) -> Result<String, String> {
        let streams = DataStreamRegistry::new();
        let stream = streams.register(single_chunk_data_stream(Bytes::from(bytes)));
        ready(read_document(stream, JobCtx::new_runtime(1, streams)))
    }

    #[test]
    fn import_consumes_real_stream_bytes_instead_of_only_reporting_a_file_name() {
        assert_eq!(
            read_fixture(INITIAL_NOTE.as_bytes().to_vec()).unwrap(),
            INITIAL_NOTE
        );
        assert_eq!(
            read_fixture(UPDATED_NOTE.as_bytes().to_vec()).unwrap(),
            UPDATED_NOTE
        );
        assert_eq!(read_fixture("café".as_bytes().to_vec()).unwrap(), "café");
    }

    #[test]
    fn imported_text_is_bounded_and_utf8_checked() {
        assert!(read_fixture(vec![b'x'; MAX_IMPORT_BYTES]).is_ok());
        assert_eq!(
            read_fixture(vec![b'x'; MAX_IMPORT_BYTES + 1]).unwrap_err(),
            "documents.error.import_limit"
        );
        assert_eq!(
            read_fixture(vec![0xff]).unwrap_err(),
            "documents.error.invalid_utf8"
        );
    }

    #[test]
    fn import_handles_remain_one_shot() {
        let streams = DataStreamRegistry::new();
        let stream = streams.register(single_chunk_data_stream(Bytes::from(
            INITIAL_NOTE.as_bytes().to_vec(),
        )));
        let ctx = JobCtx::new_runtime(1, streams);
        assert!(ready(read_document(stream, ctx.clone())).is_ok());
        assert!(ready(read_document(stream, ctx)).is_err());
    }

    #[test]
    fn beginning_and_cancelling_a_picker_preserves_previous_document_data() {
        let mut state = DocumentState {
            imported_text: Some(INITIAL_NOTE.into()),
            file_name: Some("existing.txt".into()),
            byte_len: INITIAL_NOTE.len(),
            ..Default::default()
        };
        state.begin(Phase::PickingOpen);
        state.phase = Phase::Cancelled;
        assert_eq!(state.imported_text.as_deref(), Some(INITIAL_NOTE));
        assert_eq!(state.file_name.as_deref(), Some("existing.txt"));
        assert_eq!(state.byte_len, INITIAL_NOTE.len());
    }

    #[test]
    fn print_fixture_has_two_pages_and_exact_cross_reference_offsets() {
        let pdf = std::str::from_utf8(PRINT_DOCUMENT).unwrap();
        assert!(pdf.starts_with("%PDF-1.4\n"));
        assert!(pdf.contains("/Count 2"));
        assert!(pdf.contains("PAGE 1 OF 2"));
        assert!(pdf.contains("PAGE 2 OF 2"));
        let xref = pdf.find("\nxref\n").unwrap() + 1;
        let encoded_xref: usize = pdf
            .split("startxref\n")
            .nth(1)
            .unwrap()
            .lines()
            .next()
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(xref, encoded_xref);
        for (object, entry) in pdf[xref..].lines().skip(3).take(7).enumerate() {
            let offset: usize = entry[..10].parse().unwrap();
            assert!(pdf[offset..].starts_with(&format!("{} 0 obj\n", object + 1)));
        }
    }

    #[test]
    fn translations_cover_every_diagnostic_phase_in_both_languages() {
        let env = create_env();
        let english = translation_messages(include_str!("../i18n/en-US.properties"));
        let spanish = translation_messages(include_str!("../i18n/es-ES.properties"));
        assert_eq!(
            english.keys().collect::<std::collections::BTreeSet<_>>(),
            spanish.keys().collect::<std::collections::BTreeSet<_>>()
        );
        for phase in [
            Phase::Ready,
            Phase::PickingSave,
            Phase::Writing,
            Phase::PickingOpen,
            Phase::Reading,
            Phase::Printing,
            Phase::Saved,
            Phase::Imported,
            Phase::Cancelled,
            Phase::PrintSubmitted,
            Phase::PrintPresented,
            Phase::Failed,
        ] {
            for locale in ["en-US", "es-ES"] {
                assert!(env.i18n.get(&Locale::from(locale), phase.key()).is_some());
            }
        }
    }
}
