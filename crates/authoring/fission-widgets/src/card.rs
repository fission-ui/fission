use fission_core::op::Fill;
use fission_core::ui::{CardPattern, Container, Positioned, Spacer, Widget, ZStack};
use fission_core::WidgetId;
use serde::{Deserialize, Serialize};

const IMPLICIT_CARD_ID_SALT: u32 = 0x4341_5244;

mod content;
mod description;
mod footer;
mod header;
mod layout;
mod region;
mod separator;
mod title;

pub use content::CardContent;
pub use description::CardDescription;
pub use footer::CardFooter;
pub use header::CardHeader;
pub use layout::CardLayout;
pub use title::CardTitle;

/// A compact surface that visually groups related content.
///
/// The active design system resolves the surface, border, radius, padding, and
/// optional elevation for each [`CardPattern`]. Use
/// [`CardPattern::Elevated`] when the design should request rest elevation;
/// other patterns remain flat unless their design-system recipe says otherwise.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Card {
    /// Content grouped by the card surface.
    pub child: Widget,
    /// Visual treatment: plain, raised, tinted, or explicitly elevated.
    pub pattern: CardPattern,
    /// Whether the design system's hover treatment may be shown.
    ///
    /// The treatment is resolved from the real pointer hover state; setting
    /// this flag does not make the card look permanently hovered. This changes
    /// presentation only, so compose the card with an input widget when it
    /// should dispatch an action.
    pub interactive: bool,
    /// Whether this card is currently selected.
    ///
    /// Selection is controlled by the owner and changes presentation only. It
    /// composes with the active pattern and hover treatment.
    #[serde(default)]
    pub selected: bool,
}

impl Default for Card {
    fn default() -> Self {
        Self {
            child: fission_core::ui::Row::default().into(),
            pattern: CardPattern::Raised,
            interactive: false,
            selected: false,
        }
    }
}

impl From<Card> for Widget {
    fn from(component: Card) -> Self {
        CardSurface {
            child: component.child,
            pattern: component.pattern,
            interactive: component.interactive,
            selected: component.selected,
            content_padding: CardSurfacePadding::Resolved,
        }
        .into()
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) enum CardSurfacePadding {
    Resolved,
    None,
    Explicit([f32; 4]),
}

/// Shared implementation used by legacy and sectioned card composition.
///
/// Keeping surface resolution here ensures [`Card`] and [`CardLayout`] consume
/// exactly the same active design-system recipe. Only ownership of the content
/// padding differs: a legacy card pads its one child, while a sectioned card's
/// named regions own their padding so separators can reach the surface edges.
#[derive(Clone, Debug)]
pub(super) struct CardSurface {
    pub child: Widget,
    pub pattern: CardPattern,
    pub interactive: bool,
    pub selected: bool,
    pub content_padding: CardSurfacePadding,
}

impl From<CardSurface> for Widget {
    fn from(component: CardSurface) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        let this = &component;

        let card_id = fission_core::build::current_widget_id()
            .or_else(|| fission_core::build::next_implicit_widget_id(IMPLICIT_CARD_ID_SALT))
            .unwrap_or_else(|| WidgetId::explicit("fission.widgets.card"));
        let is_hovered = this.interactive && view.runtime().interaction.is_hovered(card_id);
        let tokens = &view.env().theme.tokens;
        let theme = &view.env().theme.components.card;
        let style = theme.resolve_state(this.pattern, is_hovered, this.selected);

        let indicator_style = &theme.selected_indicator_style;
        let indicator = this.selected.then(|| {
            indicator_style
                .width
                .filter(|width| width.is_finite() && *width > 0.0)
                .zip(indicator_style.background.clone())
        });
        let indicator = indicator.flatten();
        let has_indicator = indicator.is_some();

        let child: Widget = if let Some((indicator_width, indicator_fill)) = indicator {
            let mut content = Container::new(this.child.clone());
            match this.content_padding {
                CardSurfacePadding::Resolved => {
                    content = content.padding(style.padding_box(theme.padding, theme.padding));
                }
                CardSurfacePadding::Explicit(padding) => {
                    content = content.padding(padding);
                }
                CardSurfacePadding::None => {}
            }
            let margin = indicator_style.margin.unwrap_or([0.0; 4]);
            let indicator = Container::new(Spacer::default()).bg_fill(indicator_fill);
            ZStack {
                children: vec![
                    content.into(),
                    Positioned {
                        start: Some(0.0),
                        top: Some(margin[2].max(0.0)),
                        bottom: Some(margin[3].max(0.0)),
                        width: Some(indicator_width),
                        child: Some(indicator.into()),
                        ..Default::default()
                    }
                    .into(),
                ],
                ..Default::default()
            }
            .into()
        } else {
            this.child.clone()
        };

        let mut card = Container::new(child)
            .bg_fill(
                style
                    .background
                    .clone()
                    .unwrap_or(Fill::Solid(tokens.colors.surface)),
            )
            .border_radius(style.radius.unwrap_or(theme.radius))
            .shadows(style.outer_shadows())
            .clip_overflow(true);
        if !has_indicator {
            match this.content_padding {
                CardSurfacePadding::Resolved => {
                    card = card.padding(style.padding_box(theme.padding, theme.padding));
                }
                CardSurfacePadding::Explicit(padding) => {
                    card = card.padding(padding);
                }
                CardSurfacePadding::None => {}
            }
        }
        card.id = Some(card_id);
        if let Some(border) = style.border {
            if let Fill::Solid(color) = border.fill {
                card = card.border(color, border.width);
            }
        }
        card.into()
    }
}
