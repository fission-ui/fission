//! Form fields and choice controls.

use crate::state::GalleryState;
use fission::op::BoxAlignment;
use fission::prelude::*;
use fission::widgets::{
    Dropzone, Editable, FileUpload, FormControlLayout, FormDescription, FormError, FormLabel,
    HStack, Radio, RangeSlider, VStack,
};
use fission::{PickOpenFilesRequest, PICK_OPEN_FILES};

const FIELD_WIDTH: f32 = 320.0;
const RANGE_SLIDER_WIDTH: f32 = 280.0;
const DROPZONE_HEIGHT: f32 = 140.0;
const DROPZONE_DASH: f32 = 6.0;
const PRICE_MIN: f32 = 0.0;
const PRICE_MAX: f32 = 100.0;
const PRICE_STEP: f32 = 5.0;
const DELIVERY_OPTIONS: &[(&str, &str)] = &[
    ("standard", "Standard delivery"),
    ("express", "Express delivery"),
    ("pickup", "Pick up in store"),
];

#[fission_reducer(UpdateFieldEmail)]
fn update_field_email(state: &mut GalleryState, ctx: &mut ReducerContext<GalleryState>) {
    if let Some(change) = ctx.input.text_change() {
        state.field_email = change.new_text.clone();
    }
}

#[fission_reducer(SelectRadio)]
fn select_radio(state: &mut GalleryState, index: usize) {
    state.radio_selected = index;
}

#[fission_reducer(SetPriceRange)]
fn set_price_range(state: &mut GalleryState, ctx: &mut ReducerContext<GalleryState>) {
    if let Some(change) = ctx.input.range_slider_change() {
        state.price_range = (change.start, change.end);
    }
}

#[fission_reducer(StartEditing)]
fn start_editing(state: &mut GalleryState) {
    state.editable_draft = state.editable_value.clone();
    state.editable_editing = true;
}

#[fission_reducer(UpdateEditable)]
fn update_editable(state: &mut GalleryState, ctx: &mut ReducerContext<GalleryState>) {
    if let Some(change) = ctx.input.text_change() {
        state.editable_draft = change.new_text.clone();
    }
}

#[fission_reducer(SaveEditable)]
fn save_editable(state: &mut GalleryState) {
    let draft = state.editable_draft.trim();
    if !draft.is_empty() {
        state.editable_value = draft.to_string();
    }
    state.editable_editing = false;
}

#[fission_reducer(CancelEditable)]
fn cancel_editable(state: &mut GalleryState) {
    state.editable_editing = false;
}

#[fission_reducer(BrowseFiles)]
fn browse_files(state: &mut GalleryState, ctx: &mut ReducerContext<GalleryState>) {
    state.file_upload_error = None;
    let ok: ActionEnvelope = FilesPicked.into();
    let err: ActionEnvelope = FilePickFailed.into();
    ctx.effects
        .capability(
            PICK_OPEN_FILES,
            PickOpenFilesRequest {
                allow_multiple: false,
                mime_types: Vec::new(),
                extensions: Vec::new(),
                preferred_chunk_bytes: None,
            },
        )
        .on_ok(ok)
        .on_err(err);
}

#[fission_reducer(FilesPicked)]
fn files_picked(state: &mut GalleryState, ctx: &mut ReducerContext<GalleryState>) {
    // Cancelling the picker returns no files and keeps the previous choice.
    if let Some(file) = ctx
        .input
        .capability_ok(PICK_OPEN_FILES)
        .and_then(|result| result.files.into_iter().next())
    {
        state.file_upload_name = Some(file.name);
    }
}

#[fission_reducer(FilePickFailed)]
fn file_pick_failed(state: &mut GalleryState, ctx: &mut ReducerContext<GalleryState>) {
    state.file_upload_error = Some(
        ctx.input
            .capability_error_message(PICK_OPEN_FILES)
            .unwrap_or("The file picker is not available here.")
            .to_string(),
    );
}

#[fission_reducer(FilesDropped)]
fn files_dropped(state: &mut GalleryState, ctx: &mut ReducerContext<GalleryState>) {
    let Some(paths) = ctx.input.as_drop_paths() else {
        return;
    };
    state.dropzone_files = paths
        .iter()
        .map(|path| {
            std::path::Path::new(path)
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.clone())
        })
        .collect();
}

fission_component! {
    Caption<()> { text: String } |caption, _state, cx| {} =>
    Text(caption.text.clone())
        .color(cx.view.env().theme.tokens.colors.text_secondary)
        .wrap(true)
}

fission_component! {
    FieldInput<GalleryState> |_input, state, cx| {} =>
    TextInput(
        id: Some(WidgetId::explicit("gallery.field.input")),
        semantics_identifier: Some("gallery.field.input".into()),
        value: state.field_email.clone(),
        placeholder: Some("name@company.com".into()),
        on_input: with_reducer!(cx.build, UpdateFieldEmail, update_field_email),
    )
}

pub(crate) fn field() -> Vec<Widget> {
    widgets![FieldDemo::new()]
}

fission_component! {
    FieldDemo<GalleryState> |_demo, state, cx| {
        let email = state.field_email.trim();
        let invalid = !email.is_empty() && !email.contains('@');
        let mut layout = FormControlLayout::new(
            WidgetId::explicit("gallery.field.layout"),
            FieldInput::new(),
        )
        .label(FormLabel::new("Billing email"))
        .description(FormDescription::new("Receipts and invoices go here."))
        .required(true);
        if invalid {
            layout = layout.error(FormError::new("Enter an email address with an @."));
        }
    } => Container(layout).width_length(Length::clamp(
        Length::points(0.0),
        Length::percent(100.0),
        Length::points(FIELD_WIDTH),
    ))
}

pub(crate) fn radio() -> Vec<Widget> {
    widgets![RadioDemo::new()]
}

fission_component! {
    RadioDemo<GalleryState> |_demo, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } => HStack(spacing: None, children: [
        VStack(spacing: tokens.spacing.s, children: [
            for (index, (slug, label)) in DELIVERY_OPTIONS.iter().enumerate() {
                Radio(
                    checked: state.radio_selected == index,
                    on_select: with_reducer!(cx.build, SelectRadio(index), select_radio),
                    label: (*label).to_string(),
                    key: *slug,
                ).semantics_identifier(format!("gallery.radio.{slug}"))
            }
            Radio(label: "Drone delivery (coming soon)".to_string())
                .disabled(true)
                .semantics_identifier("gallery.radio.drone"),
        ]),
    ])
}

pub(crate) fn range_slider() -> Vec<Widget> {
    widgets![RangeSliderDemo::new()]
}

fission_component! {
    RangeSliderDemo<GalleryState> |_demo, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let (start, end) = state.price_range;
    } => HStack(spacing: tokens.spacing.m, children: [
            Container(
                RangeSlider(
                    id: Some(WidgetId::explicit("gallery.range_slider")),
                    start: start,
                    end: end,
                    min: PRICE_MIN,
                    max: PRICE_MAX,
                    on_change: with_reducer!(cx.build, SetPriceRange, set_price_range),
                )
                .step(PRICE_STEP)
                .semantics_identifier("gallery.range_slider"),
            )
            .width_length(Length::clamp(
                Length::points(0.0),
                Length::percent(100.0),
                Length::points(RANGE_SLIDER_WIDTH),
            )),
            Text(format!("${start:.0} to ${end:.0}")).color(tokens.colors.text_secondary),
        ])
}

pub(crate) fn editable() -> Vec<Widget> {
    widgets![EditableDemo::new()]
}

fission_component! {
    EditableDemo<GalleryState> |_demo, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let save = with_reducer!(cx.build, SaveEditable, save_editable);
        let cancel = with_reducer!(cx.build, CancelEditable, cancel_editable);
        let value = if state.editable_editing { state.editable_draft.clone() } else { state.editable_value.clone() };
    } => Column(children: [
        HStack(spacing: tokens.spacing.s, children: [
            Container(Editable(
                id: Some(WidgetId::explicit("gallery.editable")),
                value: value,
                placeholder: "document name",
                is_editing: state.editable_editing,
                on_input: with_reducer!(cx.build, UpdateEditable, update_editable),
                on_submit: save.clone(),
                on_edit: with_reducer!(cx.build, StartEditing, start_editing),
                on_cancel: cancel.clone(),
            )).width(FIELD_WIDTH),
            if state.editable_editing {
                Button(
                variant: ButtonVariant::Filled,
                size: ComponentSize::Sm,
                child: Text("Save"),
                on_press: save.clone(),
                ).semantics_identifier("gallery.editable.save")
            }
            if state.editable_editing {
                Button(
                variant: ButtonVariant::Ghost,
                size: ComponentSize::Sm,
                child: Text("Cancel"),
                on_press: cancel.clone(),
                ).semantics_identifier("gallery.editable.cancel")
            }
        ]),
        Caption("Press the name to rename it.".to_string()),
    ])
}

pub(crate) fn file_upload() -> Vec<Widget> {
    widgets![FileUploadDemo::new()]
}

fission_component! {
    FileUploadDemo<GalleryState> |_demo, state, cx| {} => Column(children: [
        HStack(spacing: None, children: [FileUpload(
            label: "Choose file",
            selected_file: state.file_upload_name.clone(),
            error_text: None,
            on_browse: with_reducer!(cx.build, BrowseFiles, browse_files),
            browse_semantics_identifier: Some("gallery.file_upload.browse".into()),
        )]),
        if let Some(error) = &state.file_upload_error {
            Caption(error.clone())
        }
    ])
}

fission_component! {
    DropPanel<()> { message: String, highlighted: bool } |panel, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let (fill, border) = if panel.highlighted {
            (tokens.colors.primary_subtle, tokens.colors.primary)
        } else {
            (tokens.colors.surface, tokens.colors.border)
        };
    } => Container(
        Text(panel.message.clone())
            .color(tokens.colors.text_secondary)
            .wrap(true)
    )
    .width_length(Length::percent(100.0))
    .height(DROPZONE_HEIGHT)
    .padding_all(tokens.spacing.m)
    .align_child(BoxAlignment::Center)
    .bg(fill)
    .border(border, tokens.sizing.border_thick)
    .border_dash(vec![DROPZONE_DASH, DROPZONE_DASH])
    .border_radius(tokens.radii.large)
}

pub(crate) fn dropzone() -> Vec<Widget> {
    widgets![DropzoneDemo::new()]
}

fission_component! {
    DropzoneDemo<GalleryState> |_demo, state, cx| {
        let dropped = if state.dropzone_files.is_empty() {
            "Nothing dropped yet.".to_string()
        } else {
            format!("Dropped: {}", state.dropzone_files.join(", "))
        };
    } => Column(children: [
        Dropzone(
            id: Some(WidgetId::explicit("gallery.dropzone.target")),
            semantics_identifier: Some("gallery.dropzone.target".into()),
            child: DropPanel("Drag files from your computer and drop them here.".to_string(), false),
            active_child: DropPanel("Drop here to add the files.".to_string(), false),
            hover_child: DropPanel("Release to add the files.".to_string(), true),
            on_drop: with_reducer!(cx.build, FilesDropped, files_dropped),
        ),
        Caption(dropped),
    ])
}
