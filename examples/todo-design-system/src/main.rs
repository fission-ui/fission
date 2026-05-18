use fission::prelude::*;

include!(concat!(env!("OUT_DIR"), "/todo_design_system.rs"));

#[derive(Debug, Clone, PartialEq, Eq)]
struct TodoItem {
    id: usize,
    title: String,
    done: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TodoState {
    next_id: usize,
    draft: String,
    items: Vec<TodoItem>,
    theme_mode: DesignMode,
}

impl Default for TodoState {
    fn default() -> Self {
        Self {
            next_id: 4,
            draft: String::new(),
            items: vec![
                TodoItem {
                    id: 1,
                    title: "Connect the DSP package".into(),
                    done: true,
                },
                TodoItem {
                    id: 2,
                    title: "Use generated component styles".into(),
                    done: false,
                },
                TodoItem {
                    id: 3,
                    title: "Ship the themed app".into(),
                    done: false,
                },
            ],
            theme_mode: DesignMode::Light,
        }
    }
}

impl AppState for TodoState {}

#[fission_reducer(UpdateDraft)]
fn update_draft(state: &mut TodoState, value: String) {
    state.draft = value;
}

#[fission_reducer(AddTodo)]
fn add_todo(state: &mut TodoState) {
    let title = state.draft.trim();
    if title.is_empty() {
        return;
    }
    state.items.push(TodoItem {
        id: state.next_id,
        title: title.to_string(),
        done: false,
    });
    state.next_id += 1;
    state.draft.clear();
}

#[fission_reducer(ToggleTodo)]
fn toggle_todo(state: &mut TodoState, id: usize) {
    if let Some(item) = state.items.iter_mut().find(|item| item.id == id) {
        item.done = !item.done;
    }
}

#[fission_reducer(ClearDone)]
fn clear_done(state: &mut TodoState) {
    state.items.retain(|item| !item.done);
}

#[fission_reducer(SetThemeMode)]
fn set_theme_mode(state: &mut TodoState, mode: DesignMode) {
    state.theme_mode = mode;
}

struct TodoApp;

impl Widget<TodoState> for TodoApp {
    fn build(&self, ctx: &mut BuildCtx<TodoState>, view: &View<TodoState>) -> Node {
        let colors = &view.env.theme.tokens.colors;
        let spacing = &view.env.theme.tokens.spacing;
        let done_count = view.state.items.iter().filter(|item| item.done).count();
        let add = with_reducer!(ctx, AddTodo, add_todo);
        let update_draft = with_reducer!(ctx, UpdateDraft(String::new()), update_draft);
        let clear_done = with_reducer!(ctx, ClearDone, clear_done);
        let light = with_reducer!(ctx, SetThemeMode(DesignMode::Light), set_theme_mode);
        let dark = with_reducer!(ctx, SetThemeMode(DesignMode::Dark), set_theme_mode);

        let mut todo_rows = Vec::new();
        for item in &view.state.items {
            let toggle = with_reducer!(ctx, ToggleTodo(item.id), toggle_todo);
            todo_rows.push(
                Container::new(
                    Row {
                        gap: Some(12.0),
                        children: vec![
                            Checkbox {
                                checked: item.done,
                                on_toggle: Some(toggle),
                                ..Default::default()
                            }
                            .into_node(),
                            Text::new(item.title.clone())
                                .size(16.0)
                                .color(if item.done {
                                    colors.text_secondary
                                } else {
                                    colors.text_primary
                                })
                                .into_node(),
                        ],
                        ..Default::default()
                    }
                    .into_node(),
                )
                .padding([16.0, 16.0, 12.0, 12.0])
                .border(colors.border, 1.0)
                .border_radius(view.env.theme.tokens.radii.medium)
                .into_node(),
            );
        }

        let theme_toggle = Row {
            gap: Some(8.0),
            children: vec![
                Button {
                    variant: if view.state.theme_mode == DesignMode::Light {
                        ButtonVariant::Primary
                    } else {
                        ButtonVariant::SecondaryGray
                    },
                    size: ComponentSize::Sm,
                    child: Some(Box::new(Text::new("Light").into_node())),
                    on_press: Some(light),
                    ..Default::default()
                }
                .into_node(),
                Button {
                    variant: if view.state.theme_mode == DesignMode::Dark {
                        ButtonVariant::Primary
                    } else {
                        ButtonVariant::SecondaryGray
                    },
                    size: ComponentSize::Sm,
                    child: Some(Box::new(Text::new("Dark").into_node())),
                    on_press: Some(dark),
                    ..Default::default()
                }
                .into_node(),
            ],
            ..Default::default()
        }
        .into_node();

        let content = Card {
            pattern: CardPattern::Raised,
            child: Box::new(
                Column {
                    gap: Some(20.0),
                    children: vec![
                        Row {
                            gap: Some(12.0),
                            children: vec![
                                Column {
                                    gap: Some(6.0),
                                    children: vec![
                                        Text::new("Design-system todo")
                                            .size(32.0)
                                            .weight(700)
                                            .into_node(),
                                        Text::new("This example reads a DSP JSON package at build time and uses the generated Rust theme at runtime.")
                                            .color(colors.text_secondary)
                                            .into_node(),
                                    ],
                                    flex_grow: 1.0,
                                    ..Default::default()
                                }
                                .into_node(),
                                Badge {
                                    text: format!("{} done", done_count),
                                    tone: BadgeTone::Success,
                                    ..Default::default()
                                }
                                .build(ctx, view),
                            ],
                            ..Default::default()
                        }
                        .into_node(),
                        theme_toggle,
                        Row {
                            gap: Some(12.0),
                            children: vec![
                                TextInput {
                                    value: view.state.draft.clone(),
                                    placeholder: Some("Add a task".into()),
                                    on_change: Some(update_draft),
                                    width: Some(360.0),
                                    ..Default::default()
                                }
                                .into_node(),
                                Button {
                                    variant: ButtonVariant::Primary,
                                    child: Some(Box::new(Text::new("Add task").into_node())),
                                    on_press: Some(add),
                                    ..Default::default()
                                }
                                .into_node(),
                            ],
                            ..Default::default()
                        }
                        .into_node(),
                        Column {
                            gap: Some(10.0),
                            children: todo_rows,
                            ..Default::default()
                        }
                        .into_node(),
                        Button {
                            variant: ButtonVariant::TertiaryGray,
                            child: Some(Box::new(Text::new("Clear completed").into_node())),
                            on_press: Some(clear_done),
                            disabled: done_count == 0,
                            ..Default::default()
                        }
                        .into_node(),
                    ],
                    ..Default::default()
                }
                .into_node(),
            ),
            ..Default::default()
        }
        .build(ctx, view);

        Container::new(content)
            .bg(colors.background)
            .padding_all(spacing.xl)
            .into_node()
    }
}

fn main() -> anyhow::Result<()> {
    DesktopApp::new(TodoApp)
        .with_design_system::<TodoDesignSystem>(DesignMode::Light)
        .with_sync_env(|state: &TodoState, env: &mut Env| {
            env.theme = TodoDesignSystem::theme(state.theme_mode);
            env.window.title = WindowTitle::plain("Fission Todo - Design System");
        })
        .run()
}
