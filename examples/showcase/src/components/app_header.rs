use super::brand::Brand;
use crate::state::{on_open_source, on_search_changed, OpenSource, SearchChanged, ShowcaseState};
use fission::icons::material;
use fission::op::AlignItems;
use fission::prelude::*;

const COMPACT_HEADER_BREAKPOINT: f32 = 720.0;
const SEARCH_WIDTH: f32 = 280.0;

/// The app bar: where you are, finding an example, and the source code.
///
/// Preview controls live in the toolbar above the preview, next to what they change.
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

        let children = if compact {
            widgets![
                Brand,
                Container::new(search_field).flex_grow(1.0).flex_shrink(1.0),
                github,
            ]
        } else {
            widgets![
                Brand,
                Spacer {
                    flex_grow: 1.0,
                    ..Default::default()
                },
                search_field,
                github,
            ]
        };

        Container::new(Row {
            children,
            gap: Some(tokens.spacing.m),
            align_items: AlignItems::Center,
            ..Default::default()
        })
        .padding_lengths(Length::symmetric(
            Length::points(tokens.spacing.l),
            Length::points(tokens.spacing.s),
        ))
        .bg(tokens.colors.surface)
        .border_bottom(tokens.colors.border, tokens.sizing.border_hairline)
        .into()
    }
}
