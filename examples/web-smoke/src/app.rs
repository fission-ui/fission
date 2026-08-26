use fission::prelude::*;

const CONTENT_MIN_WIDTH: f32 = 240.0;
const CONTENT_MAX_WIDTH: f32 = 420.0;

#[derive(Default, Debug, Clone, PartialEq)]
pub struct CounterState {
    pub count: i32,
    pub primary_text: String,
    pub secondary_text: String,
    pub password: String,
    pub verification_code: String,
    pub primary_edits: usize,
    pub secondary_edits: usize,
}

impl GlobalState for CounterState {}

#[fission_reducer(Increment)]
fn on_increment(state: &mut CounterState) {
    state.count += 1;
}

#[fission_reducer(EditPrimary)]
fn edit_primary(state: &mut CounterState, ctx: &mut ReducerContext<CounterState>) {
    if let Some(change) = ctx.input.text_change() {
        state.primary_text = change.new_text.clone();
        state.primary_edits += 1;
    }
}

#[fission_reducer(EditSecondary)]
fn edit_secondary(state: &mut CounterState, ctx: &mut ReducerContext<CounterState>) {
    if let Some(change) = ctx.input.text_change() {
        state.secondary_text = change.new_text.clone();
        state.secondary_edits += 1;
    }
}

#[fission_reducer(EditPassword)]
fn edit_password(state: &mut CounterState, ctx: &mut ReducerContext<CounterState>) {
    if let Some(change) = ctx.input.text_change() {
        state.password = change.new_text.clone();
    }
}

#[fission_reducer(EditVerificationCode)]
fn edit_verification_code(state: &mut CounterState, ctx: &mut ReducerContext<CounterState>) {
    if let Some(change) = ctx.input.text_change() {
        state.verification_code = change.new_text.clone();
    }
}

#[derive(Clone)]
pub struct CounterApp;

impl From<CounterApp> for Widget {
    fn from(_component: CounterApp) -> Self {
        let (ctx, view) = fission::build::current::<CounterState>();
        let tokens = &view.env().theme.tokens;
        let increment = with_reducer!(ctx, Increment, on_increment);
        let edit_primary = with_reducer!(ctx, EditPrimary, edit_primary);
        let edit_secondary = with_reducer!(ctx, EditSecondary, edit_secondary);
        let edit_password = with_reducer!(ctx, EditPassword, edit_password);
        let edit_verification_code =
            with_reducer!(ctx, EditVerificationCode, edit_verification_code);

        let content = Container::new(Column {
            gap: Some(tokens.spacing.m),
            children: vec![
                Text::new("Fission web smoke")
                    .size(tokens.typography.heading_size)
                    .color(tokens.colors.text_primary)
                    .into(),
                Text::new("This exercises the shared Fission runtime through the browser shell.")
                    .size(tokens.typography.body_large_size)
                    .color(tokens.colors.text_secondary)
                    .into(),
                Text::new(format!("Count: {}", view.state().count))
                    .size(tokens.typography.font_size_xl)
                    .color(tokens.colors.primary)
                    .into(),
                Button {
                    on_press: Some(increment),
                    child: Some(Text::new("Increment").into()),
                    ..Default::default()
                }
                .semantics_identifier("web-smoke.increment")
                .into(),
                TextInput {
                    id: Some(WidgetId::explicit("web-smoke.text.primary")),
                    semantics_identifier: Some("web-smoke.text.primary".into()),
                    name: Some("primary".into()),
                    label: Some("Primary field".into()),
                    value: view.state().primary_text.clone(),
                    on_input: Some(edit_primary),
                    required: true,
                    autocorrect: true,
                    enable_suggestions: true,
                    spell_check: true,
                    ..Default::default()
                }
                .into(),
                Text::new(format!(
                    "Primary value: {} (edits: {})",
                    view.state().primary_text,
                    view.state().primary_edits
                ))
                .into(),
                TextInput {
                    id: Some(WidgetId::explicit("web-smoke.text.secondary")),
                    semantics_identifier: Some("web-smoke.text.secondary".into()),
                    name: Some("secondary".into()),
                    label: Some("Secondary field".into()),
                    value: view.state().secondary_text.clone(),
                    on_input: Some(edit_secondary),
                    ..Default::default()
                }
                .into(),
                Text::new(format!(
                    "Secondary value: {} (edits: {})",
                    view.state().secondary_text,
                    view.state().secondary_edits
                ))
                .into(),
                TextInput {
                    id: Some(WidgetId::explicit("web-smoke.text.password")),
                    semantics_identifier: Some("web-smoke.text.password".into()),
                    name: Some("password".into()),
                    label: Some("Password field".into()),
                    value: view.state().password.clone(),
                    on_input: Some(edit_password),
                    obscure_text: true,
                    autofill_hints: vec!["current-password".into()],
                    ..Default::default()
                }
                .into(),
                ZStack {
                    children: widgets![
                        Container::new(Spacer::default())
                            .width(360.0)
                            .height(66.0)
                            .bg(Color::WHITE),
                        TextInput {
                            id: Some(WidgetId::explicit("web-smoke.text.verification")),
                            semantics_identifier: Some("web-smoke.text.verification".into()),
                            value: view.state().verification_code.clone(),
                            on_input: Some(edit_verification_code),
                            width: Some(360.0),
                            height: Some(66.0),
                            padding: Some([9.0, 0.0, 2.0, 0.0]),
                            borderless: true,
                            font_family: Some("Unavailable Verification Font".into()),
                            font_size: Some(42.0),
                            line_height: Some(52.0),
                            letter_spacing: Some(22.0),
                            text_color: Some(Color::BLACK),
                            show_cursor: false,
                            keyboard_type: TextInputType::Number,
                            input_formatters: vec![
                                InputFormatter::DigitsOnly,
                                InputFormatter::SingleLine,
                            ],
                            autocorrect: false,
                            enable_suggestions: false,
                            spell_check: false,
                            smart_dashes: false,
                            smart_quotes: false,
                            autofill_hints: vec!["one-time-code".into()],
                            ..Default::default()
                        },
                    ],
                    ..Default::default()
                }
                .into(),
                Text::new(format!(
                    "Verification value: {}",
                    view.state().verification_code
                ))
                .into(),
            ],
            ..Default::default()
        })
        .width_length(Length::clamp(
            Length::points(CONTENT_MIN_WIDTH),
            Length::percent(100.0),
            Length::points(CONTENT_MAX_WIDTH),
        ))
        .into();

        Container::new(Column {
            gap: Some(tokens.spacing.none),
            children: vec![
                content,
                Spacer {
                    flex_grow: 1.0,
                    ..Default::default()
                }
                .into(),
            ],
            ..Default::default()
        })
        .width_length(Length::vw(100.0))
        .height_length(Length::vh(100.0))
        .padding_lengths(Length::all(Length::points(tokens.spacing.l)))
        .bg(tokens.colors.background)
        .into()
    }
}
