use fission::prelude::*;

const CONTENT_MIN_WIDTH: f32 = 240.0;
const CONTENT_MAX_WIDTH: f32 = 420.0;

// The verification field reproduces a one-time-code box: a white tile with a
// wide, letter-spaced font that is deliberately unavailable, so the browser
// test covers fallback metrics. A caption above the tile names it.
const CODE_FIELD_WIDTH: f32 = 360.0;
const CODE_FIELD_HEIGHT: f32 = 66.0;
const CODE_FONT_SIZE: f32 = 42.0;
const CODE_LINE_HEIGHT: f32 = 52.0;
const CODE_LETTER_SPACING: f32 = 22.0;
const CODE_PADDING: [f32; 4] = [9.0, 0.0, 2.0, 0.0];
const VERIFICATION_LABEL: &str = "Verification code";

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

fission_component! {
    pub CounterApp<CounterState>
    |_app, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
    Container(
        Scroll(
            id: Some(WidgetId::explicit("web-smoke.scroll")),
            child: Container(
                Column(
                    gap: tokens.spacing.m,
                    children: [
                        SmokeHeader(),
                        CounterSection(),
                        PrimaryField(),
                        SecondaryField(),
                        PasswordField(),
                        VerificationCodeField(),
                        PerformanceRows(),
                        ScrolledCounterButton(),
                    ],
                ),
            )
            .width_length(Length::clamp(
                Length::points(CONTENT_MIN_WIDTH),
                Length::percent(100.0),
                Length::points(CONTENT_MAX_WIDTH),
            )),
            direction: FlexDirection::Column,
            show_scrollbar: true,
            flex_grow: 1.0_f32,
            semantic_label: Some("Web smoke content".into()),
        ),
    )
    .width_length(Length::vw(100.0))
    .height_length(Length::vh(100.0))
    .padding_lengths(Length::all(Length::points(tokens.spacing.l)))
    .bg(tokens.colors.background)
}

fission_component! {
/// Keeps the browser smoke fixture large enough to expose widget-count and
/// scrolling regressions without introducing application state or animation.
    PerformanceRows<CounterState> |_rows, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Column(
            gap: tokens.spacing.xs,
            children: [
                for index in 0..120 {
                    Text(format!("Scroll qualification row {}", index + 1), key: index)
                        .size(tokens.typography.font_size_sm)
                },
            ],
        )
}

fission_component! {
/// A real control below the long fixture content. Coordinate-driven tests use
/// this to qualify hit testing after a compositor-only scroll.
    ScrolledCounterButton<CounterState> |_button, state, cx| {} =>
        Button(
            on_press: with_reducer!(cx.build, Increment, on_increment),
            child: Text(format!(
                "Increment after scrolling (Count: {})",
                state.count,
            )),
        )
        .semantics_identifier("web-smoke.increment.scrolled")
}

fission_component! {
    SmokeHeader<CounterState> |_header, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Column(
            gap: tokens.spacing.m,
            children: [
                Text("Fission web smoke")
                    .size(tokens.typography.heading_size)
                    .color(tokens.colors.text_primary),
                Text("This exercises the shared Fission runtime through the browser shell.")
                    .size(tokens.typography.body_large_size)
                    .color(tokens.colors.text_secondary),
            ],
        )
}

fission_component! {
    CounterSection<CounterState> |_section, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Column(
            gap: tokens.spacing.m,
            children: [
                Text(format!("Count: {}", state.count))
                    .size(tokens.typography.font_size_xl)
                    .color(tokens.colors.primary),
                Button(
                    on_press: with_reducer!(cx.build, Increment, on_increment),
                    child: Text("Increment"),
                )
                .semantics_identifier("web-smoke.increment"),
            ],
        )
}

fission_component! {
/// A field followed by a line echoing its value, which the browser test reads.
    EchoedField<CounterState> { field: Widget, echo: String }
    |echoed, _state, cx| {} =>
        Column(
            gap: cx.view.env().theme.tokens.spacing.m,
            children: [echoed.field, Text(echoed.echo)],
        )
}

/// Echo text for a value, reading "(empty)" rather than leaving a bare label.
fn shown(value: &str) -> &str {
    if value.is_empty() {
        "(empty)"
    } else {
        value
    }
}

fission_component! {
/// Exercises spell checking, suggestions and a required field.
    PrimaryField<CounterState> |_field, state, cx| {} =>
        EchoedField(
            TextInput(
                id: WidgetId::explicit("web-smoke.text.primary"),
                semantics_identifier: "web-smoke.text.primary".to_string(),
                name: "primary".to_string(),
                label: Some("Primary field".into()),
                placeholder: Some("Type to test spell checking".into()),
                value: state.primary_text.clone(),
                on_input: with_reducer!(cx.build, EditPrimary, edit_primary),
                required: true,
                autocorrect: true,
                enable_suggestions: true,
                spell_check: true,
            ),
            format!(
                "Primary value: {} (edits: {})",
                shown(&state.primary_text),
                state.primary_edits,
            ),
        )
}

fission_component! {
    SecondaryField<CounterState> |_field, state, cx| {} =>
        EchoedField(
            TextInput(
                id: WidgetId::explicit("web-smoke.text.secondary"),
                semantics_identifier: "web-smoke.text.secondary".to_string(),
                name: "secondary".to_string(),
                label: Some("Secondary field".into()),
                placeholder: Some("Type anything".into()),
                value: state.secondary_text.clone(),
                on_input: with_reducer!(cx.build, EditSecondary, edit_secondary),
            ),
            format!(
                "Secondary value: {} (edits: {})",
                shown(&state.secondary_text),
                state.secondary_edits,
            ),
        )
}

fission_component! {
/// Exercises obscured input and password autofill.
    PasswordField<CounterState> |_field, state, cx| {} =>
        TextInput(
            id: WidgetId::explicit("web-smoke.text.password"),
            semantics_identifier: "web-smoke.text.password".to_string(),
            name: "password".to_string(),
            label: Some("Password field".into()),
            placeholder: Some("Enter a password".into()),
            value: state.password.clone(),
            on_input: with_reducer!(cx.build, EditPassword, edit_password),
            obscure_text: true,
            autofill_hints: vec!["current-password".into()],
        )
}

fission_component! {
/// Exercises a borderless, digits-only one-time-code field drawn over its own tile.
///
/// The field has no built-in label (one would shift it off the tile), so a
/// caption sits above the tile and the same text names the field semantically.
    VerificationCodeField<CounterState> |_field, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let code = state.verification_code.clone();
    } =>
        EchoedField(
            Column(
                gap: tokens.spacing.xs,
                children: [
                    Text(VERIFICATION_LABEL)
                        .size(tokens.typography.label_large_size)
                        .color(tokens.colors.text_primary),
                    ZStack(
                        children: [
                            Container(Spacer())
                                .width(CODE_FIELD_WIDTH)
                                .height(CODE_FIELD_HEIGHT)
                                .bg(Color::WHITE),
                            TextInput(
                    id: WidgetId::explicit("web-smoke.text.verification"),
                    semantics_identifier: "web-smoke.text.verification".to_string(),
                    semantics: Semantics {
                        label: Some(VERIFICATION_LABEL.into()),
                        ..Default::default()
                    },
                    // Words, not sample digits, so the hint can't pass for a typed code.
                    placeholder: Some("6-digit code".into()),
                    value: code.clone(),
                    on_input: with_reducer!(
                        cx.build,
                        EditVerificationCode,
                        edit_verification_code
                    ),
                    width: CODE_FIELD_WIDTH,
                    height: CODE_FIELD_HEIGHT,
                    padding: CODE_PADDING,
                    borderless: true,
                    font_family: "Unavailable Verification Font".to_string(),
                    font_size: CODE_FONT_SIZE,
                    line_height: CODE_LINE_HEIGHT,
                    letter_spacing: CODE_LETTER_SPACING,
                    text_color: Color::BLACK,
                    show_cursor: false,
                    keyboard_type: TextInputType::Number,
                    input_formatters: vec![InputFormatter::DigitsOnly, InputFormatter::SingleLine],
                    autocorrect: false,
                    enable_suggestions: false,
                    spell_check: false,
                    smart_dashes: false,
                    smart_quotes: false,
                    autofill_hints: vec!["one-time-code".into()],
                            ),
                        ],
                    ),
                ],
            ),
            format!("Verification value: {}", shown(&code)),
        )
}
