use fission::prelude::*;

const CONTENT_MIN_WIDTH: f32 = 240.0;
const CONTENT_MAX_WIDTH: f32 = 420.0;

// The verification field reproduces a one-time-code box: a white tile with a
// wide, letter-spaced font that is deliberately unavailable, so the browser
// test covers fallback metrics.
const CODE_FIELD_WIDTH: f32 = 360.0;
const CODE_FIELD_HEIGHT: f32 = 66.0;
const CODE_FONT_SIZE: f32 = 42.0;
const CODE_LINE_HEIGHT: f32 = 52.0;
const CODE_LETTER_SPACING: f32 = 22.0;
const CODE_PADDING: [f32; 4] = [9.0, 0.0, 2.0, 0.0];

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
        let (_, view) = fission::build::current::<CounterState>();
        let tokens = &view.env().theme.tokens;
        let content = Container::new(Column {
            gap: Some(tokens.spacing.m),
            children: widgets![
                SmokeHeader,
                CounterSection,
                PrimaryField,
                SecondaryField,
                PasswordField,
                VerificationCodeField,
            ],
            ..Default::default()
        })
        .width_length(Length::clamp(
            Length::points(CONTENT_MIN_WIDTH),
            Length::percent(100.0),
            Length::points(CONTENT_MAX_WIDTH),
        ));

        Container::new(Column {
            gap: Some(tokens.spacing.none),
            children: widgets![
                content,
                Spacer {
                    flex_grow: 1.0,
                    ..Default::default()
                },
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

struct SmokeHeader;

impl From<SmokeHeader> for Widget {
    fn from(_header: SmokeHeader) -> Self {
        let (_, view) = fission::build::current::<CounterState>();
        let tokens = &view.env().theme.tokens;
        Column {
            gap: Some(tokens.spacing.m),
            children: widgets![
                Text::new("Fission web smoke")
                    .size(tokens.typography.heading_size)
                    .color(tokens.colors.text_primary),
                Text::new("This exercises the shared Fission runtime through the browser shell.")
                    .size(tokens.typography.body_large_size)
                    .color(tokens.colors.text_secondary),
            ],
            ..Default::default()
        }
        .into()
    }
}

struct CounterSection;

impl From<CounterSection> for Widget {
    fn from(_section: CounterSection) -> Self {
        let (ctx, view) = fission::build::current::<CounterState>();
        let tokens = &view.env().theme.tokens;
        Column {
            gap: Some(tokens.spacing.m),
            children: widgets![
                Text::new(format!("Count: {}", view.state().count))
                    .size(tokens.typography.font_size_xl)
                    .color(tokens.colors.primary),
                Button {
                    on_press: Some(with_reducer!(ctx, Increment, on_increment)),
                    child: Some(Text::new("Increment").into()),
                    ..Default::default()
                }
                .semantics_identifier("web-smoke.increment"),
            ],
            ..Default::default()
        }
        .into()
    }
}

/// A field followed by a line echoing its value, which the browser test reads.
struct EchoedField {
    field: Widget,
    echo: String,
}

impl From<EchoedField> for Widget {
    fn from(echoed: EchoedField) -> Self {
        let (_, view) = fission::build::current::<CounterState>();
        Column {
            gap: Some(view.env().theme.tokens.spacing.m),
            children: widgets![echoed.field, Text::new(echoed.echo)],
            ..Default::default()
        }
        .into()
    }
}

/// Exercises spell checking, suggestions and a required field.
struct PrimaryField;

impl From<PrimaryField> for Widget {
    fn from(_field: PrimaryField) -> Self {
        let (ctx, view) = fission::build::current::<CounterState>();
        let state = view.state();
        EchoedField {
            field: TextInput {
                id: Some(WidgetId::explicit("web-smoke.text.primary")),
                semantics_identifier: Some("web-smoke.text.primary".into()),
                name: Some("primary".into()),
                label: Some("Primary field".into()),
                value: state.primary_text.clone(),
                on_input: Some(with_reducer!(ctx, EditPrimary, edit_primary)),
                required: true,
                autocorrect: true,
                enable_suggestions: true,
                spell_check: true,
                ..Default::default()
            }
            .into(),
            echo: format!(
                "Primary value: {} (edits: {})",
                state.primary_text, state.primary_edits
            ),
        }
        .into()
    }
}

struct SecondaryField;

impl From<SecondaryField> for Widget {
    fn from(_field: SecondaryField) -> Self {
        let (ctx, view) = fission::build::current::<CounterState>();
        let state = view.state();
        EchoedField {
            field: TextInput {
                id: Some(WidgetId::explicit("web-smoke.text.secondary")),
                semantics_identifier: Some("web-smoke.text.secondary".into()),
                name: Some("secondary".into()),
                label: Some("Secondary field".into()),
                value: state.secondary_text.clone(),
                on_input: Some(with_reducer!(ctx, EditSecondary, edit_secondary)),
                ..Default::default()
            }
            .into(),
            echo: format!(
                "Secondary value: {} (edits: {})",
                state.secondary_text, state.secondary_edits
            ),
        }
        .into()
    }
}

/// Exercises obscured input and password autofill.
struct PasswordField;

impl From<PasswordField> for Widget {
    fn from(_field: PasswordField) -> Self {
        let (ctx, view) = fission::build::current::<CounterState>();
        TextInput {
            id: Some(WidgetId::explicit("web-smoke.text.password")),
            semantics_identifier: Some("web-smoke.text.password".into()),
            name: Some("password".into()),
            label: Some("Password field".into()),
            value: view.state().password.clone(),
            on_input: Some(with_reducer!(ctx, EditPassword, edit_password)),
            obscure_text: true,
            autofill_hints: vec!["current-password".into()],
            ..Default::default()
        }
        .into()
    }
}

/// Exercises a borderless, digits-only one-time-code field drawn over its own tile.
struct VerificationCodeField;

impl From<VerificationCodeField> for Widget {
    fn from(_field: VerificationCodeField) -> Self {
        let (ctx, view) = fission::build::current::<CounterState>();
        let code = view.state().verification_code.clone();
        EchoedField {
            field: ZStack {
                children: widgets![
                    Container::new(Spacer::default())
                        .width(CODE_FIELD_WIDTH)
                        .height(CODE_FIELD_HEIGHT)
                        .bg(Color::WHITE),
                    TextInput {
                        id: Some(WidgetId::explicit("web-smoke.text.verification")),
                        semantics_identifier: Some("web-smoke.text.verification".into()),
                        value: code.clone(),
                        on_input: Some(with_reducer!(
                            ctx,
                            EditVerificationCode,
                            edit_verification_code
                        )),
                        width: Some(CODE_FIELD_WIDTH),
                        height: Some(CODE_FIELD_HEIGHT),
                        padding: Some(CODE_PADDING),
                        borderless: true,
                        font_family: Some("Unavailable Verification Font".into()),
                        font_size: Some(CODE_FONT_SIZE),
                        line_height: Some(CODE_LINE_HEIGHT),
                        letter_spacing: Some(CODE_LETTER_SPACING),
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
            echo: format!("Verification value: {}", code),
        }
        .into()
    }
}
