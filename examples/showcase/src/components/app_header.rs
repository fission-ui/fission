use super::brand::Brand;
use crate::state::{
    on_open_source, on_set_locale, on_toggle_locale_menu, OpenSource, SetLocale, ShowcaseState,
    ToggleLocaleMenu,
};
use fission::icons::material;
use fission::op::AlignItems;
use fission::prelude::*;
use fission::widgets::{Select, SelectItem, Tooltip};

/// Wide enough for the longest language name, since the menu matches its trigger.
const LOCALE_PICKER_WIDTH: f32 = 128.0;

/// Locale codes and their names, written in their own language so they are not translated.
const LOCALES: [(&str, &str); 2] = [("en-US", "English"), ("es-ES", "Español")];

/// The repository host, named on the button so the destination is not a guess.
const GITHUB_LABEL: &str = "GitHub";

fission_component! {
/// The app bar: where you are, language, and the source code.
///
/// Language changes the whole showcase, so it sits with the app rather than in
/// the toolbar above the preview, which keeps that toolbar to one row. Search
/// lives at the top of the catalog it filters.
    #[derive(Clone, Debug)] pub(crate) AppHeader<ShowcaseState> {}
    |_component, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Container(Row(
            children: [
                Brand(),
                Spacer(flex_grow: 1.0_f32),
                LocalePicker(),
                Tooltip(
                    id: WidgetId::explicit("showcase.github.tooltip"),
                    child: Button(
                        variant: ButtonVariant::Ghost,
                        size: ComponentSize::Sm,
                        content: ButtonContent::new(TextContent::Literal(GITHUB_LABEL.into()))
                            .leading_icon(Icon::svg(material::action::code::round())),
                        on_press: with_reducer!(
                            cx.build,
                            OpenSource("https://github.com/fission-ui/fission".into()),
                            on_open_source
                        ),
                    )
                    .semantics_identifier("showcase.github"),
                    text: cx.view.tr("showcase.github.hint"),
                    is_visible: false,
                    motion: None,
                )
            ],
            gap: tokens.spacing.m,
            align_items: AlignItems::Center,
        ))
            .padding_lengths(Length::symmetric(
                Length::points(tokens.spacing.l),
                Length::points(tokens.spacing.s),
            ))
            .bg(tokens.colors.surface)
            .border_bottom(tokens.colors.border, tokens.sizing.border_hairline)
}

fission_component! {
/// Chooses the language of the showcase and the preview.
///
/// Keeps the `showcase.preview.locale` identifier the toolbar control used, so
/// automation that switches language still finds it.
    #[derive(Clone, Copy, Debug)] LocalePicker<ShowcaseState> {}
    |_component, state, cx| {
        let items: Vec<SelectItem> = LOCALES
            .iter()
            .map(|(code, name)| SelectItem {
                label: (*name).to_string(),
                icon: None,
                on_select: with_reducer!(cx.build, SetLocale((*code).to_string()), on_set_locale),
                semantics_identifier: Some(format!("showcase.preview.locale.{code}")),
            })
            .collect();
        let selected = LOCALES
            .iter()
            .find(|(code, _)| *code == state.locale.0.as_str())
            .map(|(_, name)| (*name).to_string());

    } =>
        SemanticsRegion(Select(
            id: WidgetId::explicit("showcase.preview.locale.select"),
            selected_label: selected,
            items: items,
            is_open: state.locale_open,
            on_toggle: with_reducer!(cx.build, ToggleLocaleMenu, on_toggle_locale_menu),
            trigger_semantics_identifier: "showcase.preview.locale.trigger".to_string(),
            placeholder: cx.view.tr("showcase.workbench.locale"),
            width: LOCALE_PICKER_WIDTH,
        ))
        .role(Role::Group)
        .label(cx.view.tr("showcase.workbench.locale"))
        .identifier("showcase.preview.locale")
}
