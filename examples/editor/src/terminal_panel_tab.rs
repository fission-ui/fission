use crate::layout::ACTIVE_INDICATOR_THICKNESS;
use crate::model::{BottomPanelTab, EditorState};
use crate::palette::EditorPalette;
use fission::prelude::*;

fission_component! {
    pub TerminalPanelTab<EditorState> {
        label: &'static str,
        active: bool,
        panel: BottomPanelTab,
        id: &'static str,
    } |tab, _state, cx| {
        let palette = EditorPalette::from_theme(&cx.view.env().theme);
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Button(
            id: Some(WidgetId::explicit(tab.id)),
            variant: ButtonVariant::Ghost,
            child: VStack(
                    spacing: tokens.spacing.none,
                    children: [
                        Container(
                            Text(tab.label)
                                .size(tokens.typography.font_size_xs)
                                .color(if tab.active {
                                    palette.bright_text
                                } else {
                                    palette.dim_text
                            }),
                        )
                        .padding_all(tokens.spacing.s),
                        Container(Spacer())
                            .height(ACTIVE_INDICATOR_THICKNESS)
                            .bg(if tab.active {
                                palette.bright_text
                            } else {
                                palette.transparent
                            }),
                    ],
            ),
            on_press: |tab, state, _cx| {
                state.bottom_panel_tab = tab.panel;
                if tab.panel == BottomPanelTab::Terminal {
                    state.ensure_terminal_session();
                }
            },
            padding: [tokens.spacing.none, tokens.spacing.none, tokens.spacing.none, tokens.spacing.none],
        )
        .semantics_identifier(format!("editor.bottom-panel.{}", tab.label.to_lowercase()))
}
