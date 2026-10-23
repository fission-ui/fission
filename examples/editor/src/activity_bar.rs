use crate::layout::ACTIVITY_BAR_WIDTH;
use crate::model::*;
use crate::palette::EditorPalette;
use fission::prelude::*;

fission_component! {
    pub(crate) ActivityBar<EditorState> |_bar, state, cx| {
        let palette = EditorPalette::from_theme(&cx.view.env().theme);
        let tokens = &cx.view.env().theme.tokens;
        let section_icons = [
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
    } =>
        Container(
            Column(
                children: [
                    for (icon, section, label) in section_icons {
                        ActivityButton(icon, section, label, key: label)
                    },
                ],
            ),
        )
        .width(ACTIVITY_BAR_WIDTH)
        .bg(palette.activity_bar_bg)
        .flex_shrink(0.0)
}

fission_component! {
    ActivityButton<EditorState> {
        icon: &'static str,
        section: SidebarSection,
        label: &'static str,
    } |button, state, cx| {
        let palette = EditorPalette::from_theme(&cx.view.env().theme);
        let tokens = &cx.view.env().theme.tokens;
        let active = state.sidebar_visible && state.sidebar_section == button.section;
    } =>
    Button(
        variant: ButtonVariant::Ghost,
        child: Container(
            Align::new(
                Icon::svg(button.icon)
                    .size(tokens.typography.font_size_xl)
                    .color(if active { palette.bright_text } else { palette.dim_text }),
            ),
        ).border(if active { palette.bright_text } else { palette.transparent }, 0.0),
        on_press: |button, state, _cx| {
            if state.sidebar_visible && state.sidebar_section == button.section {
                state.sidebar_visible = false;
            } else {
                state.sidebar_section = button.section;
                state.sidebar_visible = true;
            }
        },
        width: ACTIVITY_BAR_WIDTH,
        height: ACTIVITY_BAR_WIDTH,
        padding: [tokens.spacing.none, tokens.spacing.none, tokens.spacing.none, tokens.spacing.none],
        semantics: Semantics {
            label: Some(button.label.into()),
            ..Default::default()
        },
    )
}
