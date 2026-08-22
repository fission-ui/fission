use super::brand::Brand;
use super::nav_items::NavItems;
use super::preview_toolbar::PreviewToolbar;
use crate::semantics::ShowcaseSemantics;
use crate::state::{on_open_source, on_search_changed, OpenSource, SearchChanged, ShowcaseState};
use fission::icons::material;
use fission::op::{AlignItems, Fill, JustifyContent};
use fission::prelude::*;

const COMPACT_HEADER_BREAKPOINT: f32 = 920.0;

#[derive(Clone, Debug)]
pub(crate) struct AppHeader;

impl From<AppHeader> for Widget {
    fn from(_component: AppHeader) -> Self {
        let (_ctx, view) = fission::build::current::<ShowcaseState>();
        if view.viewport_size().width < COMPACT_HEADER_BREAKPOINT {
            CompactHeader.into()
        } else {
            ExpandedHeader.into()
        }
    }
}

#[derive(Clone, Debug)]
struct ExpandedHeader;

impl From<ExpandedHeader> for Widget {
    fn from(_component: ExpandedHeader) -> Self {
        let (ctx, view) = fission::build::current::<ShowcaseState>();
        let tokens = &view.env().theme.tokens;
        let search = with_reducer!(ctx, SearchChanged, on_search_changed);
        let open_github = with_reducer!(
            ctx,
            OpenSource("https://github.com/fission-ui/fission".into()),
            on_open_source
        );

        Container::new(Row {
            children: widgets![
                Brand,
                NavItems,
                Spacer {
                    flex_grow: 1.0,
                    ..Default::default()
                },
                PreviewToolbar,
                TextInput {
                    id: Some(WidgetId::explicit("showcase.search")),
                    semantics_identifier: Some("showcase.search".into()),
                    value: view.state().search.clone(),
                    placeholder: Some(TextContent::Key("showcase.nav.search".into())),
                    on_input: Some(search),
                    width: Some(tokens.spacing.xxxxl * 2.6),
                    ..Default::default()
                },
                Button {
                    variant: ButtonVariant::TertiaryGray,
                    size: ComponentSize::Sm,
                    child: Some(
                        Icon::svg(material::action::code::round())
                            .size(tokens.typography.font_size_lg)
                            .into(),
                    ),
                    on_press: Some(open_github),
                    semantics: Some(Semantics::link("GitHub").identifier("showcase.github"),),
                    ..Default::default()
                },
            ],
            gap: Some(tokens.spacing.m),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Start,
            ..Default::default()
        })
        .padding_lengths(Length::all(Length::points(tokens.spacing.m)))
        .bg_fill(Fill::Solid(tokens.colors.surface.with_alpha(246)))
        .border(tokens.colors.divider, 1.0)
        .into()
    }
}

#[derive(Clone, Debug)]
struct CompactHeader;

impl From<CompactHeader> for Widget {
    fn from(_component: CompactHeader) -> Self {
        let (ctx, view) = fission::build::current::<ShowcaseState>();
        let tokens = &view.env().theme.tokens;
        let search = with_reducer!(ctx, SearchChanged, on_search_changed);
        Container::new(Column {
            children: widgets![
                Row {
                    children: widgets![
                        Brand,
                        Spacer {
                            flex_grow: 1.0,
                            ..Default::default()
                        },
                    ],
                    align_items: AlignItems::Center,
                    ..Default::default()
                },
                PreviewToolbar,
                TextInput {
                    id: Some(WidgetId::explicit("showcase.search.compact")),
                    semantics_identifier: Some("showcase.search".into()),
                    value: view.state().search.clone(),
                    placeholder: Some(TextContent::Key("showcase.nav.search".into())),
                    on_input: Some(search),
                    ..Default::default()
                },
            ],
            gap: Some(tokens.spacing.s),
            ..Default::default()
        })
        .padding_lengths(Length::all(Length::points(tokens.spacing.s)))
        .bg(tokens.colors.surface)
        .border(tokens.colors.divider, 1.0)
        .into()
    }
}
