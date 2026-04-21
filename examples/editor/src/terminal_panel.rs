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

        let terminal_tab_color = if view.state.bottom_panel_tab == BottomPanelTab::Terminal {
            Color { r: 255, g: 255, b: 255, a: 255 }
        } else {
            Color { r: 180, g: 180, b: 180, a: 255 }
        };
        let problems_tab_color = if view.state.bottom_panel_tab == BottomPanelTab::Problems {
            Color { r: 255, g: 255, b: 255, a: 255 }
        } else {
            Color { r: 180, g: 180, b: 180, a: 255 }
        };

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

        let header = Container::new(
            HStack {
                spacing: Some(16.0),
                children: vec![
                    Button {
                        variant: ButtonVariant::Ghost,
                        child: Some(Box::new(Text::new("TERMINAL").size(12.0).color(terminal_tab_color).into_node())),
                        on_press: Some(set_terminal),
                        padding: Some([4.0, 4.0, 0.0, 0.0]),
                        ..Default::default()
                    }.into_node(),
                    Button {
                        variant: ButtonVariant::Ghost,
                        child: Some(Box::new(Text::new("PROBLEMS").size(12.0).color(problems_tab_color).into_node())),
                        on_press: Some(set_problems),
                        padding: Some([4.0, 4.0, 0.0, 0.0]),
                        ..Default::default()
                    }.into_node(),
                    Spacer { flex_grow: 1.0, ..Default::default() }.into_node(),
                ],
            }.into_node(),
        )
        .bg(header_bg)
        .height(24.0)
        .padding_all(4.0)
        .flex_shrink(0.0)
        .into_node();

        let content = if view.state.bottom_panel_tab == BottomPanelTab::Terminal {
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

        let submit = ctx.bind(
            SubmitTerminalCommand,
            (|s: &mut EditorState, _, _| s.run_terminal_command())
                as Handler<EditorState, SubmitTerminalCommand>,
        );

        let mut lines = Vec::new();
        for line in &view.state.terminal_lines {
            lines.push(
                Text::new(line.clone()).size(13.0).color(text_color).into_node(),
            );
        }

        let output = Container::new(
            Scroll {
                direction: FlexDirection::Column,
                child: Some(Box::new(
                    VStack { spacing: Some(2.0), children: lines }.into_node(),
                )),
                show_scrollbar: true,
                flex_grow: 1.0,
                flex_shrink: 1.0,
                ..Default::default()
            }.into_node(),
        )
        .bg(bg)
        .padding_all(4.0)
        .flex_grow(1.0)
        .into_node();

        let input_row = Container::new(
            HStack {
                spacing: Some(4.0),
                children: vec![
                    Text::new("$").size(13.0).color(Color { r: 80, g: 200, b: 80, a: 255 }).into_node(),
                    TextInput {
                        value: view.state.terminal_input.clone(),
                        placeholder: Some("Type a command...".into()),
                        on_change: Some(update_input),
                        ..Default::default()
                    }.into_node(),
                    Button {
                        variant: ButtonVariant::Ghost,
                        child: Some(Box::new(Text::new("Run").size(12.0).color(text_color).into_node())),
                        on_press: Some(submit),
                        height: Some(28.0),
                        ..Default::default()
                    }.into_node(),
                ],
            }.into_node(),
        )
        .bg(Color { r: 30, g: 30, b: 30, a: 255 })
        .padding_all(4.0)
        .flex_shrink(0.0)
        .into_node();

        fission_core::ui::Column {
            children: vec![output, input_row],
            flex_grow: 1.0,
            ..Default::default()
        }.into_node()
    }
}
