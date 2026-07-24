use crate::layout::ACTIVITY_BAR_WIDTH;
use crate::model::*;
use crate::palette::{ACTIVITY_BAR_BG, BRIGHT_TEXT, DIM_TEXT, TRANSPARENT};
use fission::core::ui::{Align, Button, ButtonVariant, Column, Container, Widget};
use fission::core::{reduce_with, ActionEnvelope};

pub(crate) struct ActivityBar;

impl From<ActivityBar> for Widget {
    fn from(_component: ActivityBar) -> Self {
        let (ctx, view) = fission::build::current::<EditorState>();
        let tokens = &view.env().theme.tokens;

        let section_icons = vec![
            (
                fission::icons::material::action::description::round(),
                SidebarSection::Explorer,
                "Explorer",
            ),
            (
                fission::icons::material::action::search::round(),
                SidebarSection::Search,
                "Search",
            ),
            (
                fission::icons::material::action::commit::round(),
                SidebarSection::Git,
                "Source Control",
            ),
            (
                fission::icons::material::action::extension::round(),
                SidebarSection::Extensions,
                "Extensions",
            ),
        ];

        let set_section_id = ctx
            .bind(
                SetSidebarSection(SidebarSection::Explorer),
                reduce_with!(
                    (|s: &mut EditorState, a: SetSidebarSection, _| {
                        if s.sidebar_visible && s.sidebar_section == a.0 {
                            s.sidebar_visible = false;
                        } else {
                            s.sidebar_section = a.0;
                            s.sidebar_visible = true;
                        }
                    })
                ),
            )
            .id;

        let mut icons = Vec::new();
        for (icon_svg, section, _label) in &section_icons {
            let is_active =
                view.state().sidebar_visible && view.state().sidebar_section == *section;
            let color = if is_active { BRIGHT_TEXT } else { DIM_TEXT };

            let indicator_color = if is_active { BRIGHT_TEXT } else { TRANSPARENT };

            icons.push(
                Button {
                    variant: ButtonVariant::Ghost,
                    child: Some(
                        Container::new(Align::new(
                            fission::widgets::Icon::svg(*icon_svg)
                                .size(tokens.typography.font_size_xl)
                                .color(color),
                        ))
                        .border(indicator_color, 0.0)
                        .into(),
                    ),
                    on_press: Some(ActionEnvelope {
                        id: set_section_id,
                        payload: serde_json::to_vec(&SetSidebarSection(*section)).unwrap(),
                    }),
                    width: Some(ACTIVITY_BAR_WIDTH),
                    height: Some(ACTIVITY_BAR_WIDTH),
                    padding: Some([tokens.spacing.none; 4]),
                    ..Default::default()
                }
                .into(),
            );
        }

        Container::new(Column {
            children: icons,
            ..Default::default()
        })
        .width(ACTIVITY_BAR_WIDTH)
        .bg(ACTIVITY_BAR_BG)
        .flex_shrink(0.0)
        .into()
    }
}
