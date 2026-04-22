use crate::model::{BottomPanelTab, EditorState, SubmitTerminalCommand, UpdateTerminalInput};
use fission_core::op::Color;
use fission_core::ui::{Button, ButtonVariant, Container, Node, Scroll, Text, TextInput};
use fission_core::{BuildCtx, FlexDirection, Handler, View, Widget};
use fission_widgets::{VStack, HStack, Spacer};

pub struct TerminalPanel;

impl Widget<EditorState> for TerminalPanel {
    fn build(&self, ctx: &mut BuildCtx<EditorState>, view: &View<EditorState>) -> Node {
        let text_color = Color { r: 204, g: 204, b: 204, a: 255 };
        let bg = Color { r: 24, g: 24, b: 24, a: 255 };
        let header_bg = Color { r: 37, g: 37, b: 38, a: 255 };
        let border_color = Color { r: 48, g: 48, b: 49, a: 255 };

        let is_terminal = view.state.bottom_panel_tab == BottomPanelTab::Terminal;
        let is_problems = view.state.bottom_panel_tab == BottomPanelTab::Problems;

        let set_terminal = ctx.bind(
            crate::model::Noop,
            (|s: &mut EditorState, _, _| s.bottom_panel_tab = BottomPanelTab::Terminal)
                as Handler<EditorState, crate::model::Noop>,
        );
        let set_problems = ctx.bind(
            crate::model::Noop,
            (|s: &mut EditorState, _, _| s.bottom_panel_tab = BottomPanelTab::Problems)
                as Handler<EditorState, crate::model::Noop>,
        );

        // Tab bar with underline indicators
        let tab = |label: &str, active: bool, action: fission_core::ActionEnvelope| -> Node {
            let label_color = if active {
                Color { r: 255, g: 255, b: 255, a: 255 }
            } else {
                Color { r: 150, g: 150, b: 150, a: 255 }
            };

            let underline = if active {
                Container::new(Spacer::default().into_node())
                    .height(2.0)
                    .bg(Color { r: 255, g: 255, b: 255, a: 255 })
                    .into_node()
            } else {
                Container::new(Spacer::default().into_node())
                    .height(2.0)
                    .bg(Color { r: 0, g: 0, b: 0, a: 0 })
                    .into_node()
            };

            Button {
                variant: ButtonVariant::Ghost,
                child: Some(Box::new(
                    VStack {
                        spacing: Some(0.0),
                        children: vec![
                            Container::new(
                                Text::new(label).size(11.0).color(label_color).into_node(),
                            ).padding_all(6.0).into_node(),
                            underline,
                        ],
                    }.into_node(),
                )),
                on_press: Some(action),
                padding: Some([0.0; 4]),
                ..Default::default()
            }.into_node()
        };

        let header = Container::new(
            HStack {
                spacing: Some(0.0),
                children: vec![
                    tab("TERMINAL", is_terminal, set_terminal),
                    tab("PROBLEMS", is_problems, set_problems),
                    Spacer { flex_grow: 1.0, ..Default::default() }.into_node(),
                ],
            }.into_node(),
        )
        .bg(header_bg)
        .height(28.0)
        .border(border_color, 1.0)
        .flex_shrink(0.0)
        .into_node();

        let content = if is_terminal {
            self.build_terminal(ctx, view, bg, text_color)
        } else {
            crate::diagnostics_panel::DiagnosticsPanel.build(ctx, view)
        };

        Container::new(
            fission_core::ui::Column {
                children: vec![header, content],
                flex_grow: 1.0,
                ..Default::default()
            }.into_node(),
        )
        .height(view.state.terminal_height)
        .bg(bg)
        .flex_shrink(0.0)
        .into_node()
    }
}

impl TerminalPanel {
    fn build_terminal(&self, ctx: &mut BuildCtx<EditorState>, view: &View<EditorState>, bg: Color, text_color: Color) -> Node {
        let update_input = ctx.bind(
            UpdateTerminalInput(String::new()),
            (|s: &mut EditorState, a: UpdateTerminalInput, _| s.terminal_input = a.0)
                as Handler<EditorState, UpdateTerminalInput>,
        );

        let prompt_color = Color { r: 80, g: 200, b: 80, a: 255 };

        let prompt_bg = Color { r: 30, g: 30, b: 30, a: 255 };
        let separator_color = Color { r: 58, g: 58, b: 58, a: 255 };
        let green_prompt = Color { r: 50, g: 220, b: 50, a: 255 };

        // Build output lines (history)
        let mut output_lines = Vec::new();
        for line in &view.state.terminal_lines {
            output_lines.push(
                Text::new(line.clone()).size(13.0).color(text_color).into_node(),
            );
        }

        let output_scroll = Container::new(
            Scroll {
                direction: FlexDirection::Column,
                child: Some(Box::new(
                    VStack { spacing: Some(2.0), children: output_lines }.into_node(),
                )),
                show_scrollbar: true,
                flex_grow: 1.0,
                flex_shrink: 1.0,
                ..Default::default()
            }.into_node(),
        )
        .bg(bg)
        .padding_all(6.0)
        .flex_grow(1.0)
        .into_node();

        // Thin separator between output and prompt
        let prompt_separator = Container::new(Spacer::default().into_node())
            .height(1.0)
            .bg(separator_color)
            .flex_shrink(0.0)
            .into_node();

        // Current prompt line: $ <inline input>
        let prompt_row = Container::new(
            HStack {
                spacing: Some(4.0),
                children: vec![
                    Text::new("$").size(13.0).color(green_prompt).into_node(),
                    TextInput {
                        value: view.state.terminal_input.clone(),
                        placeholder: Some("".into()),
                        on_change: Some(update_input),
                        borderless: true,
                        ..Default::default()
                    }.into_node(),
                ],
            }.into_node(),
        )
        .bg(prompt_bg)
        .padding_all(6.0)
        .flex_shrink(0.0)
        .into_node();

        fission_core::ui::Column {
            children: vec![output_scroll, prompt_separator, prompt_row],
            flex_grow: 1.0,
            ..Default::default()
        }
        .into_node()
    }
}
