use fission_core::op::Fill;
use fission_core::ui::{ComponentSize, Container, Widget};

#[derive(Clone, Copy, Debug)]
pub(super) enum CardRegionKind {
    Header,
    Content,
    Footer,
}

#[derive(Clone, Debug)]
pub(super) struct CardRegion {
    pub child: Widget,
    pub kind: CardRegionKind,
    pub size: ComponentSize,
    pub include_vertical_padding: bool,
}

impl From<CardRegion> for Widget {
    fn from(component: CardRegion) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        let theme = &view.env().theme.components.card;
        let density = theme.resolve_size(component.size);
        let region = match component.kind {
            CardRegionKind::Header => &theme.header_style,
            CardRegionKind::Content => &theme.content_style,
            CardRegionKind::Footer => &theme.footer_style,
        };
        let style = density.merge(region);
        let mut padding = style.padding_box(theme.padding, theme.padding);
        if !component.include_vertical_padding {
            padding[2] = 0.0;
            padding[3] = 0.0;
        }

        let mut container = Container::new(component.child)
            .padding(padding)
            .shadows(style.outer_shadows());
        if let Some(background) = style.background {
            container = container.bg_fill(background);
        }
        if let Some(radius) = style.radius {
            container = container.border_radius(radius);
        }
        if !matches!(component.kind, CardRegionKind::Footer) {
            if let Some(border) = style.border {
                if let Fill::Solid(color) = border.fill {
                    container = container.border(color, border.width);
                }
            }
        }
        container.into()
    }
}
