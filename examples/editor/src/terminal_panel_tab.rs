use crate::layout::ACTIVE_INDICATOR_THICKNESS;
use crate::model::EditorState;
use crate::palette::{BRIGHT_TEXT, DIM_TEXT, TRANSPARENT};
use fission::prelude::*;
use fission::widgets::{Spacer, VStack};

pub struct TerminalPanelTab {
    pub label: &'static str,
    pub active: bool,
    pub action: ActionEnvelope,
    pub id: &'static str,
}

impl From<TerminalPanelTab> for Widget {
    fn from(tab: TerminalPanelTab) -> Self {
        let (_, view) = fission::build::current::<EditorState>();
        let tokens = &view.env().theme.tokens;

        Button {
            id: Some(WidgetId::explicit(tab.id)),
            variant: ButtonVariant::Ghost,
            child: Some(
                VStack {
                    spacing: Some(tokens.spacing.none),
                    children: widgets![
                        Container::new(
                            Text::new(tab.label)
                                .size(tokens.typography.font_size_xs)
                                .color(if tab.active { BRIGHT_TEXT } else { DIM_TEXT }),
                        )
                        .padding_all(tokens.spacing.s),
                        Container::new(Spacer::default())
                            .height(ACTIVE_INDICATOR_THICKNESS)
                            .bg(if tab.active { BRIGHT_TEXT } else { TRANSPARENT }),
                    ],
                }
                .into(),
            ),
            on_press: Some(tab.action),
            padding: Some([tokens.spacing.none; 4]),
            ..Default::default()
        }
        .semantics_identifier(format!("editor.bottom-panel.{}", tab.label.to_lowercase()))
        .into()
    }
}
