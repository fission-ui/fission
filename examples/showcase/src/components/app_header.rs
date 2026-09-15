use super::brand::Brand;
use crate::state::{
    on_open_source, on_search_changed, on_set_locale, on_toggle_locale_menu, OpenSource,
    SearchChanged, SetLocale, ShowcaseState, ToggleLocaleMenu,
};
use fission::icons::material;
use fission::op::AlignItems;
use fission::prelude::*;
use fission::widgets::{Select, SelectItem};

const COMPACT_HEADER_BREAKPOINT: f32 = 720.0;
const SEARCH_WIDTH: f32 = 280.0;
/// Wide enough for the longest language name, since the menu matches its trigger.
const LOCALE_PICKER_WIDTH: f32 = 128.0;

/// Locale codes and their names, written in their own language so they are not translated.
const LOCALES: [(&str, &str); 2] = [("en-US", "English"), ("es-ES", "Español")];

/// The app bar: where you are, finding an example, language, and the source code.
///
/// Language changes the whole showcase, so it sits with the app rather than in
/// the toolbar above the preview, which keeps that toolbar to one row.
#[derive(Clone, Debug)]
pub(crate) struct AppHeader;

impl From<AppHeader> for Widget {
    fn from(_component: AppHeader) -> Self {
        let (ctx, view) = fission::build::current::<ShowcaseState>();
        let tokens = &view.env().theme.tokens;
        let compact = view.viewport_size().width < COMPACT_HEADER_BREAKPOINT;
        let search = with_reducer!(ctx, SearchChanged, on_search_changed);
        let open_github = with_reducer!(
            ctx,
            OpenSource("https://github.com/fission-ui/fission".into()),
            on_open_source
        );

        let search_field: Widget = TextInput {
            id: Some(WidgetId::explicit("showcase.search")),
            semantics_identifier: Some("showcase.search".into()),
            value: view.state().search.clone(),
            placeholder: Some(TextContent::Key("showcase.nav.search".into())),
            on_input: Some(search),
            width: (!compact).then_some(SEARCH_WIDTH),
            ..Default::default()
        }
        .into();
        let github: Widget = Button {
            variant: ButtonVariant::Ghost,
            size: ComponentSize::Sm,
            icon_content: Some(ButtonIconContent::new(
                Icon::svg(material::action::code::round()),
                "GitHub",
            )),
            on_press: Some(open_github),
            ..Default::default()
        }
        .semantics_identifier("showcase.github")
        .into();

        let spacer = || Spacer {
            flex_grow: 1.0,
            ..Default::default()
        };
        // A narrow bar has no room for search beside the brand and language, so
        // search takes a full-width row of its own underneath.
        let content: Widget = if compact {
            Column {
                gap: Some(tokens.spacing.s),
                children: widgets![
                    Row {
                        children: widgets![Brand, spacer(), LocalePicker, github],
                        gap: Some(tokens.spacing.m),
                        align_items: AlignItems::Center,
                        ..Default::default()
                    },
                    Container::new(search_field).width_length(Length::percent(100.0)),
                ],
                ..Default::default()
            }
            .into()
        } else {
            Row {
                children: widgets![Brand, spacer(), search_field, LocalePicker, github],
                gap: Some(tokens.spacing.m),
                align_items: AlignItems::Center,
                ..Default::default()
            }
            .into()
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
