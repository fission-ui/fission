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

/// The app bar: where you are, language, and the source code.
///
/// Language changes the whole showcase, so it sits with the app rather than in
/// the toolbar above the preview, which keeps that toolbar to one row. Search
/// lives at the top of the catalog it filters.
#[derive(Clone, Debug)]
pub(crate) struct AppHeader;

impl From<AppHeader> for Widget {
    fn from(_component: AppHeader) -> Self {
        let (ctx, view) = fission::build::current::<ShowcaseState>();
        let tokens = &view.env().theme.tokens;
        let open_github = with_reducer!(
            ctx,
            OpenSource("https://github.com/fission-ui/fission".into()),
            on_open_source
        );

        // A bare `<>` glyph is guesswork: nothing about it says the repository,
        // and the icon set ships no GitHub mark to use instead. The destination
        // is named on the button, with the tooltip saying what pressing it does.
        let github: Widget = Tooltip {
            id: WidgetId::explicit("showcase.github.tooltip"),
            child: Button {
                variant: ButtonVariant::Ghost,
                size: ComponentSize::Sm,
                content: Some(
                    // A product name, so it reads the same in every language.
                    ButtonContent::new(TextContent::Literal(GITHUB_LABEL.into()))
                        .leading_icon(Icon::svg(material::action::code::round())),
                ),
                on_press: Some(open_github),
                ..Default::default()
            }
            .semantics_identifier("showcase.github")
            .into(),
            text: view.tr("showcase.github.hint"),
            is_visible: false,
            motion: None,
        }
        .into();

        let content = Row {
            children: widgets![
                Brand,
                Spacer {
                    flex_grow: 1.0,
                    ..Default::default()
                },
                LocalePicker,
                github
            ],
            gap: Some(tokens.spacing.m),
            align_items: AlignItems::Center,
            ..Default::default()
        };

        Container::new(content)
            .padding_lengths(Length::symmetric(
                Length::points(tokens.spacing.l),
                Length::points(tokens.spacing.s),
            ))
            .bg(tokens.colors.surface)
            .border_bottom(tokens.colors.border, tokens.sizing.border_hairline)
            .into()
    }
}

/// Chooses the language of the showcase and the preview.
///
/// Keeps the `showcase.preview.locale` identifier the toolbar control used, so
/// automation that switches language still finds it.
#[derive(Clone, Copy, Debug)]
struct LocalePicker;

impl From<LocalePicker> for Widget {
    fn from(_component: LocalePicker) -> Self {
        let (ctx, view) = fission::build::current::<ShowcaseState>();
        let state = view.state();
        let items = LOCALES
            .iter()
            .map(|(code, name)| SelectItem {
                label: (*name).to_string(),
                icon: None,
                on_select: with_reducer!(ctx, SetLocale((*code).to_string()), on_set_locale),
                semantics_identifier: Some(format!("showcase.preview.locale.{code}")),
            })
            .collect();
        let selected = LOCALES
            .iter()
            .find(|(code, _)| *code == state.locale.0.as_str())
            .map(|(_, name)| (*name).to_string());

        SemanticsRegion::new(Select {
            id: WidgetId::explicit("showcase.preview.locale.select"),
            selected_label: selected,
            items,
            is_open: state.locale_open,
            on_toggle: Some(with_reducer!(ctx, ToggleLocaleMenu, on_toggle_locale_menu)),
            trigger_semantics_identifier: Some("showcase.preview.locale.trigger".into()),
            placeholder: view.tr("showcase.workbench.locale"),
            width: Some(LOCALE_PICKER_WIDTH),
        })
        .role(Role::Group)
        .label(view.tr("showcase.workbench.locale"))
        .identifier("showcase.preview.locale")
        .into()
    }
}
