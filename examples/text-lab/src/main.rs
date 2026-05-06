use anyhow::Result;
use fission_core::{
    ActionEnvelope, AppState, BuildCtx, Handler, NodeId, ReducerContext, View, Widget, WidgetNodeId,
};
use fission_shell_desktop::DesktopApp;
use fission_widgets::{
    Button, ButtonVariant, Combobox, Container, FocusScope, FormControl, HStack, MenuButton,
    MenuItem, Modal, ModalAction, Node, SafeArea, Scroll, Spacer, Text, TextInput, VStack,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
struct TextLabState {
    single_line: String,
    multiline: String,
    inline_combobox: String,
    modal_to: String,
    modal_subject: String,
    modal_body: String,
    show_modal: bool,
    menu_open: bool,
    status: String,
}

impl AppState for TextLabState {}

#[derive(fission_macros::Action, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
struct SetSingleLine(String);

#[derive(fission_macros::Action, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
struct SetMultiline(String);

#[derive(fission_macros::Action, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
struct SetInlineCombobox(String);

#[derive(fission_macros::Action, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
struct SetModalTo(String);

#[derive(fission_macros::Action, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
struct SetModalSubject(String);

#[derive(fission_macros::Action, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
struct SetModalBody(String);

#[derive(fission_macros::Action, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
struct SetShowModal(bool);

#[derive(fission_macros::Action, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
struct SetMenuOpen(bool);

#[derive(fission_macros::Action, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
struct MenuPicked(String);

#[derive(fission_macros::Action, Debug, Clone, PartialEq, Serialize, Deserialize)]
struct ApplyModal;

fn on_set_single_line(
    state: &mut TextLabState,
    action: SetSingleLine,
    _ctx: &mut ReducerContext<TextLabState>,
) {
    state.single_line = action.0;
}

fn on_set_multiline(
    state: &mut TextLabState,
    action: SetMultiline,
    _ctx: &mut ReducerContext<TextLabState>,
) {
    state.multiline = action.0;
}

fn on_set_inline_combobox(
    state: &mut TextLabState,
    action: SetInlineCombobox,
    _ctx: &mut ReducerContext<TextLabState>,
) {
    state.inline_combobox = action.0;
}

fn on_set_modal_to(
    state: &mut TextLabState,
    action: SetModalTo,
    _ctx: &mut ReducerContext<TextLabState>,
) {
    state.modal_to = action.0;
}

fn on_set_modal_subject(
    state: &mut TextLabState,
    action: SetModalSubject,
    _ctx: &mut ReducerContext<TextLabState>,
) {
    state.modal_subject = action.0;
}

fn on_set_modal_body(
    state: &mut TextLabState,
    action: SetModalBody,
    _ctx: &mut ReducerContext<TextLabState>,
) {
    state.modal_body = action.0;
}

fn on_set_show_modal(
    state: &mut TextLabState,
    action: SetShowModal,
    _ctx: &mut ReducerContext<TextLabState>,
) {
    state.show_modal = action.0;
}

fn on_set_menu_open(
    state: &mut TextLabState,
    action: SetMenuOpen,
    _ctx: &mut ReducerContext<TextLabState>,
) {
    state.menu_open = action.0;
}

fn on_menu_picked(
    state: &mut TextLabState,
    action: MenuPicked,
    _ctx: &mut ReducerContext<TextLabState>,
) {
    state.status = format!("Menu action: {}", action.0);
    state.menu_open = false;
}

fn on_apply_modal(
    state: &mut TextLabState,
    _action: ApplyModal,
    _ctx: &mut ReducerContext<TextLabState>,
) {
    state.status = format!(
        "Modal applied: to='{}' subject='{}' body_len={}",
        state.modal_to,
        state.modal_subject,
        state.modal_body.len()
    );
    state.show_modal = false;
}

fn filtered_suggestions(query: &str, values: &[&str]) -> Vec<String> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Vec::new();
    }

    values
        .iter()
        .filter(|value| value.to_lowercase().contains(&q))
        .map(|value| (*value).to_string())
        .collect()
}

struct TextLabApp;

impl Widget<TextLabState> for TextLabApp {
    fn build(&self, ctx: &mut BuildCtx<TextLabState>, view: &View<TextLabState>) -> Node {
        let viewport_width = view.viewport_size().width.max(0.0);
        let content_width = (viewport_width - 56.0).clamp(280.0, 640.0);
        let modal_field_width = (content_width - 40.0).clamp(260.0, 600.0);

        let set_single_line_id = ctx
            .bind(
                SetSingleLine(String::new()),
                on_set_single_line as Handler<TextLabState, SetSingleLine>,
            )
            .id;
        let set_multiline_id = ctx
            .bind(
                SetMultiline(String::new()),
                on_set_multiline as Handler<TextLabState, SetMultiline>,
            )
            .id;
        let set_inline_combobox_id = ctx
            .bind(
                SetInlineCombobox(String::new()),
                on_set_inline_combobox as Handler<TextLabState, SetInlineCombobox>,
            )
            .id;
        let set_modal_to_id = ctx
            .bind(
                SetModalTo(String::new()),
                on_set_modal_to as Handler<TextLabState, SetModalTo>,
            )
            .id;
        let set_modal_subject_id = ctx
            .bind(
                SetModalSubject(String::new()),
                on_set_modal_subject as Handler<TextLabState, SetModalSubject>,
            )
            .id;
        let set_modal_body_id = ctx
            .bind(
                SetModalBody(String::new()),
                on_set_modal_body as Handler<TextLabState, SetModalBody>,
            )
            .id;
        let set_show_modal_id = ctx
            .bind(
                SetShowModal(false),
                on_set_show_modal as Handler<TextLabState, SetShowModal>,
            )
            .id;
        let set_menu_open_id = ctx
            .bind(
                SetMenuOpen(false),
                on_set_menu_open as Handler<TextLabState, SetMenuOpen>,
            )
            .id;
        let menu_picked_id = ctx
            .bind(
                MenuPicked(String::new()),
                on_menu_picked as Handler<TextLabState, MenuPicked>,
            )
            .id;
        let apply_modal = ctx.bind(
            ApplyModal,
            on_apply_modal as Handler<TextLabState, ApplyModal>,
        );

        let inline_options = [
            "alice@example.com",
            "bob@example.com",
            "carol@example.com",
            "design@fission.rs",
            "ops@fission.rs",
            "team@fission.rs",
        ];
        let inline_items = filtered_suggestions(&view.state.inline_combobox, &inline_options);
        let inline_has_exact = inline_options
            .iter()
            .any(|value| value.eq_ignore_ascii_case(view.state.inline_combobox.trim()));

        let modal_options = [
            "alice@example.com",
            "bob@example.com",
            "qa@fission.rs",
            "team@fission.rs",
        ];
        let modal_items = filtered_suggestions(&view.state.modal_to, &modal_options);
        let modal_has_exact = modal_options
            .iter()
            .any(|value| value.eq_ignore_ascii_case(view.state.modal_to.trim()));

        let menu_toggle = ActionEnvelope {
            id: set_menu_open_id,
            payload: serde_json::to_vec(&SetMenuOpen(!view.state.menu_open)).unwrap(),
        };

        let open_modal = ActionEnvelope {
            id: set_show_modal_id,
            payload: serde_json::to_vec(&SetShowModal(true)).unwrap(),
        };

        let close_modal = ActionEnvelope {
            id: set_show_modal_id,
            payload: serde_json::to_vec(&SetShowModal(false)).unwrap(),
        };

        let single_line_input_id = NodeId::derived(
            WidgetNodeId::explicit("text_lab_single_line").as_u128(),
            &[],
        );
        let body_input_id =
            NodeId::derived(WidgetNodeId::explicit("text_lab_multiline").as_u128(), &[]);
        let modal_subject_input_id = NodeId::derived(
            WidgetNodeId::explicit("text_lab_modal_subject").as_u128(),
            &[],
        );
        let modal_body_input_id =
            NodeId::derived(WidgetNodeId::explicit("text_lab_modal_body").as_u128(), &[]);

        let content = VStack {
            spacing: Some(14.0),
            children: vec![
                Text::new("Text Lab")
                    .size(28.0)
                    .into_node(),
                Text::new("Use this harness to validate text-input behavior, wrappers, and event latency traces.")
                    .size(13.0)
                    .into_node(),
                FormControl {
                    id: None,
                    label: Some("Single-line input".to_string()),
                    required: false,
                    error: None,
                    helper: Some("Try rapid typing, navigation, and selection.".to_string()),
                    child: Box::new(
                        TextInput {
                            id: Some(single_line_input_id),
                            value: view.state.single_line.clone(),
                            placeholder: Some("Type quickly here".into()),
                            on_change: Some(ActionEnvelope {
                                id: set_single_line_id,
                                payload: Vec::new(),
                            }),
                            width: Some(content_width),
                            ..Default::default()
                        }
                        .into_node(),
                    ),
                }
                .build(ctx, view),
                FormControl {
                    id: None,
                    label: Some("Multiline input".to_string()),
                    required: false,
                    error: None,
                    helper: Some("Use enter, arrow keys, and drag selection.".to_string()),
                    child: Box::new(
                        TextInput {
                            id: Some(body_input_id),
                            value: view.state.multiline.clone(),
                            placeholder: Some("Multiline editing area".into()),
                            on_change: Some(ActionEnvelope {
                                id: set_multiline_id,
                                payload: Vec::new(),
                            }),
                            multiline: true,
                            width: Some(content_width),
                            height: Some(120.0),
                            ..Default::default()
                        }
                        .into_node(),
                    ),
                }
                .build(ctx, view),
                FormControl {
                    id: None,
                    label: Some("Combobox wrapper".to_string()),
                    required: false,
                    error: None,
                    helper: Some("Type to open suggestions and pick via mouse/keyboard.".to_string()),
                    child: Box::new(
                        Combobox {
                            id: WidgetNodeId::explicit("text_lab_inline_combobox"),
                            value: view.state.inline_combobox.clone(),
                            items: inline_items,
                            is_open: !view.state.inline_combobox.trim().is_empty() && !inline_has_exact,
                            width: Some(content_width),
                            max_popup_height: Some(180.0),
                            on_change: Some(ActionEnvelope {
                                id: set_inline_combobox_id,
                                payload: Vec::new(),
                            }),
                            on_select: Some(Arc::new(move |value| ActionEnvelope {
                                id: set_inline_combobox_id,
                                payload: serde_json::to_vec(&SetInlineCombobox(value)).unwrap(),
                            })),
                            on_toggle: None,
                        }
                        .build(ctx, view),
                    ),
                }
                .build(ctx, view),
                HStack {
                    spacing: Some(10.0),
                    children: vec![
                        MenuButton {
                            id: WidgetNodeId::explicit("text_lab_menu_button"),
                            label: "Actions".to_string(),
                            is_open: view.state.menu_open,
                            on_toggle: Some(menu_toggle),
                            items: vec![
                                MenuItem {
                                    label: "Mark all as read".to_string(),
                                    icon: None,
                                    on_select: Some(ActionEnvelope {
                                        id: menu_picked_id,
                                        payload: serde_json::to_vec(&MenuPicked("mark_all_read".to_string()))
                                            .unwrap(),
                                    }),
                                },
                                MenuItem {
                                    label: "Archive selected".to_string(),
                                    icon: None,
                                    on_select: Some(ActionEnvelope {
                                        id: menu_picked_id,
                                        payload: serde_json::to_vec(&MenuPicked("archive_selected".to_string()))
                                            .unwrap(),
                                    }),
                                },
                            ],
                        }
                        .build(ctx, view),
                        Button {
                            variant: ButtonVariant::Filled,
                            child: Some(Box::new(Text::new("Open modal text flow").into_node())),
                            on_press: Some(open_modal),
                            ..Default::default()
                        }
                        .into_node(),
                    ],
                }
                .build(ctx, view),
                Spacer {
                    width: None,
                    height: Some(6.0),
                    ..Default::default()
                }
                .into_node(),
                Text::new(format!("Status: {}", view.state.status))
                    .size(13.0)
                    .into_node(),
            ],
        }
        .build(ctx, view);

        let modal_content = if view.state.show_modal {
            FocusScope {
                id: None,
                is_barrier: true,
                children: vec![VStack {
                    spacing: Some(10.0),
                    children: vec![
                        FormControl {
                            id: None,
                            label: Some("To".to_string()),
                            required: true,
                            error: None,
                            helper: None,
                            child: Box::new(
                                Combobox {
                                    id: WidgetNodeId::explicit("text_lab_modal_to"),
                                    value: view.state.modal_to.clone(),
                                    items: modal_items,
                                    is_open: !view.state.modal_to.trim().is_empty()
                                        && !modal_has_exact,
                                    width: Some(modal_field_width),
                                    max_popup_height: Some(180.0),
                                    on_change: Some(ActionEnvelope {
                                        id: set_modal_to_id,
                                        payload: Vec::new(),
                                    }),
                                    on_select: Some(Arc::new(move |value| ActionEnvelope {
                                        id: set_modal_to_id,
                                        payload: serde_json::to_vec(&SetModalTo(value)).unwrap(),
                                    })),
                                    on_toggle: None,
                                }
                                .build(ctx, view),
                            ),
                        }
                        .build(ctx, view),
                        FormControl {
                            id: None,
                            label: Some("Subject".to_string()),
                            required: false,
                            error: None,
                            helper: None,
                            child: Box::new(
                                TextInput {
                                    id: Some(modal_subject_input_id),
                                    value: view.state.modal_subject.clone(),
                                    placeholder: Some("Subject".into()),
                                    on_change: Some(ActionEnvelope {
                                        id: set_modal_subject_id,
                                        payload: Vec::new(),
                                    }),
                                    width: Some(modal_field_width),
                                    ..Default::default()
                                }
                                .into_node(),
                            ),
                        }
                        .build(ctx, view),
                        FormControl {
                            id: None,
                            label: Some("Body".to_string()),
                            required: true,
                            error: None,
                            helper: Some(
                                "Exercise multiline and popup interactions here.".to_string(),
                            ),
                            child: Box::new(
                                TextInput {
                                    id: Some(modal_body_input_id),
                                    value: view.state.modal_body.clone(),
                                    placeholder: Some("Type a longer message".into()),
                                    on_change: Some(ActionEnvelope {
                                        id: set_modal_body_id,
                                        payload: Vec::new(),
                                    }),
                                    multiline: true,
                                    width: Some(modal_field_width),
                                    height: Some(180.0),
                                    ..Default::default()
                                }
                                .into_node(),
                            ),
                        }
                        .build(ctx, view),
                    ],
                }
                .build(ctx, view)],
            }
            .into_node()
        } else {
            fission_core::ui::widgets::spacer::Spacer::default().into_node()
        };

        let modal = Modal {
            id: WidgetNodeId::explicit("text_lab_modal"),
            title: "Text Lab Modal".to_string(),
            is_open: view.state.show_modal,
            on_dismiss: Some(close_modal.clone()),
            width: Some((viewport_width - 48.0).clamp(320.0, 720.0)),
            actions: vec![
                ModalAction {
                    label: "Cancel".to_string(),
                    on_press: Some(close_modal),
                    is_primary: false,
                },
                ModalAction {
                    label: "Apply".to_string(),
                    on_press: Some(apply_modal),
                    is_primary: true,
                },
            ],
            content: Box::new(modal_content),
        }
        .build(ctx, view);

        SafeArea {
            id: None,
            child: Box::new(
                Scroll {
                    child: Some(Box::new(
                        Container::new(
                            VStack {
                                spacing: Some(0.0),
                                children: vec![content, modal],
                            }
                            .into_node(),
                        )
                        .padding_all(16.0)
                        .into_node(),
                    )),
                    show_scrollbar: true,
                    flex_grow: 1.0,
                    ..Default::default()
                }
                .into_node(),
            ),
        }
        .into_node()
    }
}

fn main() -> Result<()> {
    DesktopApp::new(TextLabApp).run()
}
