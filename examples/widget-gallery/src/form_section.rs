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

fn caption(text: impl Into<String>) -> Widget {
    let (_, view) = fission::build::current::<()>();
    Text::new(text.into())
        .color(view.env().theme.tokens.colors.text_secondary)
        .wrap(true)
        .into()
}

pub(crate) fn field() -> Vec<Widget> {
    let (ctx, view) = fission::build::current::<GalleryState>();
    let state = view.state();
    let email = state.field_email.trim();
    let invalid = !email.is_empty() && !email.contains('@');
    let mut layout = FormControlLayout::new(
        WidgetId::explicit("gallery.field.layout"),
        TextInput {
            id: Some(WidgetId::explicit("gallery.field.input")),
            semantics_identifier: Some("gallery.field.input".into()),
            value: state.field_email.clone(),
            placeholder: Some("name@company.com".into()),
            on_input: Some(with_reducer!(ctx, UpdateFieldEmail, update_field_email)),
            ..Default::default()
        },
    )
    .label(FormLabel::new("Billing email"))
    .description(FormDescription::new("Receipts and invoices go here."))
    .required(true);
    if invalid {
        layout = layout.error(FormError::new("Enter an email address with an @."));
    }
    widgets![Container::new(layout).width_length(Length::clamp(
        Length::points(0.0),
        Length::percent(100.0),
        Length::points(FIELD_WIDTH),
    ))]
}

pub(crate) fn radio() -> Vec<Widget> {
    let (ctx, view) = fission::build::current::<GalleryState>();
    let state = view.state();
    let tokens = &view.env().theme.tokens;
    let options: Vec<Widget> = DELIVERY_OPTIONS
        .iter()
        .enumerate()
        .map(|(index, (slug, label))| {
            Radio {
                checked: state.radio_selected == index,
                on_select: Some(with_reducer!(ctx, SelectRadio(index), select_radio)),
                label: Some((*label).into()),
                ..Default::default()
            }
            .semantics_identifier(format!("gallery.radio.{slug}"))
            .into()
        })
        .chain(std::iter::once(
            Radio {
                label: Some("Drone delivery (coming soon)".into()),
                ..Default::default()
            }
            .disabled(true)
            .semantics_identifier("gallery.radio.drone")
            .into(),
        ))
        .collect();
    widgets![HStack {
        spacing: None,
        children: widgets![VStack {
            spacing: Some(tokens.spacing.s),
            children: options,
        }],
    }]
}

pub(crate) fn range_slider() -> Vec<Widget> {
    let (ctx, view) = fission::build::current::<GalleryState>();
    let state = view.state();
    let tokens = &view.env().theme.tokens;
    let (start, end) = state.price_range;
    widgets![HStack {
        spacing: Some(tokens.spacing.m),
        children: widgets![
            Container::new(
                RangeSlider {
                    id: Some(WidgetId::explicit("gallery.range_slider")),
                    start,
                    end,
                    min: PRICE_MIN,
                    max: PRICE_MAX,
                    on_change: Some(with_reducer!(ctx, SetPriceRange, set_price_range)),
                    ..Default::default()
                }
                .step(PRICE_STEP)
                .semantics_identifier("gallery.range_slider"),
            )
            .width_length(Length::clamp(
                Length::points(0.0),
                Length::percent(100.0),
                Length::points(RANGE_SLIDER_WIDTH),
            )),
            Text::new(format!("${start:.0} to ${end:.0}")).color(tokens.colors.text_secondary),
        ],
    }]
}

pub(crate) fn editable() -> Vec<Widget> {
    let (ctx, view) = fission::build::current::<GalleryState>();
    let state = view.state();
    let tokens = &view.env().theme.tokens;
    let save = with_reducer!(ctx, SaveEditable, save_editable);
    let cancel = with_reducer!(ctx, CancelEditable, cancel_editable);
    let value = if state.editable_editing {
        state.editable_draft.clone()
    } else {
        state.editable_value.clone()
    };
    let mut children = vec![Container::new(Editable {
        id: Some(WidgetId::explicit("gallery.editable")),
        value,
        placeholder: "document name".into(),
        is_editing: state.editable_editing,
        on_input: Some(with_reducer!(ctx, UpdateEditable, update_editable)),
        on_submit: Some(save.clone()),
        on_edit: Some(with_reducer!(ctx, StartEditing, start_editing)),
        on_cancel: Some(cancel.clone()),
    })
    .width(FIELD_WIDTH)
    .into()];
    // The editor does not submit on Enter yet, so explicit buttons accept or discard the edit.
    if state.editable_editing {
        children.push(
            Button {
                variant: ButtonVariant::Filled,
                size: ComponentSize::Sm,
                child: Some(Text::new("Save").into()),
                on_press: Some(save),
                ..Default::default()
            }
            .semantics_identifier("gallery.editable.save")
            .into(),
        );
        children.push(
            Button {
                variant: ButtonVariant::Ghost,
                size: ComponentSize::Sm,
                child: Some(Text::new("Cancel").into()),
                on_press: Some(cancel),
                ..Default::default()
            }
            .semantics_identifier("gallery.editable.cancel")
            .into(),
        );
    }
    vec![
        HStack {
            spacing: Some(tokens.spacing.s),
            children,
        }
        .into(),
        caption("Press the name to rename it."),
    ]
}

pub(crate) fn file_upload() -> Vec<Widget> {
    let (ctx, view) = fission::build::current::<GalleryState>();
    let state = view.state();
    let mut children = widgets![HStack {
        spacing: None,
        children: widgets![FileUpload {
            label: "Choose file".into(),
            selected_file: state.file_upload_name.clone(),
            on_browse: Some(with_reducer!(ctx, BrowseFiles, browse_files)),
            browse_semantics_identifier: Some("gallery.file_upload.browse".into()),
        }],
    }];
    if let Some(error) = &state.file_upload_error {
        children.push(caption(error.clone()));
    }
    children
}

fn drop_panel(message: &str, highlighted: bool) -> Widget {
    let (_, view) = fission::build::current::<()>();
    let tokens = &view.env().theme.tokens;
    let (fill, border) = if highlighted {
        (tokens.colors.primary_subtle, tokens.colors.primary)
    } else {
        (tokens.colors.surface, tokens.colors.border)
    };
    Container::new(
        Text::new(message)
            .color(tokens.colors.text_secondary)
            .wrap(true),
    )
    .width_length(Length::percent(100.0))
    .height(DROPZONE_HEIGHT)
    .padding_all(tokens.spacing.m)
    .align_child(BoxAlignment::Center)
    .bg(fill)
    .border(border, tokens.sizing.border_thick)
    .border_dash(vec![DROPZONE_DASH, DROPZONE_DASH])
    .border_radius(tokens.radii.large)
    .into()
}

pub(crate) fn dropzone() -> Vec<Widget> {
    let (ctx, view) = fission::build::current::<GalleryState>();
    let state = view.state();
    let dropped = if state.dropzone_files.is_empty() {
        "Nothing dropped yet.".to_string()
    } else {
        format!("Dropped: {}", state.dropzone_files.join(", "))
    };
    vec![
        Dropzone {
            id: Some(WidgetId::explicit("gallery.dropzone.target")),
            semantics_identifier: Some("gallery.dropzone.target".into()),
            child: drop_panel("Drag files from your computer and drop them here.", false),
            active_child: Some(drop_panel("Drop here to add the files.", false)),
            hover_child: Some(drop_panel("Release to add the files.", true)),
            on_drop: Some(with_reducer!(ctx, FilesDropped, files_dropped)),
            on_drag_enter: None,
            on_drag_leave: None,
        }
        .into(),
        caption(dropped),
    ]
}
