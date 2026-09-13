use crate::motion_support::{
    collapse_y_in, dedupe, exit_for, fade_in, presence_active, push_enter_with_exit,
    resolve_motion, slot_id, SLOT_CONTENT, SLOT_HEADER, SLOT_INDICATOR, SLOT_PANEL,
};
use crate::stack::{HStack, VStack};
use crate::Icon;
use fission_core::motion::{
    deg, Motion, MotionEasing, MotionPropertyId, MotionStartValue, MotionTrack, MotionTransition,
    Presence,
};
use fission_core::op::Fill;
use fission_core::ui::{
    Button, ButtonContentAlign, ButtonVariant, Container, SemanticsRegion, Text, TextContent,
    Widget,
};
use fission_core::{ActionEnvelope, WidgetId};
use fission_icons::material;
use fission_ir::Role;
use serde::{Deserialize, Serialize};
use std::ops::Add;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
/// Optional motion presets owned by [`Accordion`].
///
/// Accordions render opened/closed panels immediately unless
/// [`Accordion::motion`] is set. Presets lower to presence tracks for the
/// stable `panel` slot and motion tracks for the optional `indicator` slot.
///
/// ```rust,ignore
/// let motion = Some(
///     AccordionMotion::Collapse + AccordionMotion::Fade + AccordionMotion::Chevron,
/// );
/// ```
pub enum AccordionMotion {
    /// No accordion-owned motion.
    None,
    /// Curated default: collapse, fade, and chevron rotation.
    Default,
    /// Animate panel height.
    Collapse,
    /// Fade panel content.
    Fade,
    /// Rotate the indicator.
    Chevron,
    /// Compound collapse plus fade preset.
    FadeCollapse,
    /// Compound collapse, fade, and chevron preset.
    FadeCollapseWithChevron,
    /// Ordered composition of accordion motion atoms.
    Composition(Vec<AccordionMotion>),
    /// Caller-provided tracks for accordion stable slots.
    Custom {
        /// Panel enter tracks.
        panel_enter: Vec<MotionTrack>,
        /// Panel exit tracks.
        panel_exit: Vec<MotionTrack>,
        /// Optional indicator tracks.
        indicator: Option<Vec<MotionTrack>>,
        /// Whether the panel remains rendered after exit completes.
        keep_rendered: bool,
    },
}

impl AccordionMotion {
    /// Flattens and normalizes an ordered accordion-motion composition.
    pub fn compose(items: impl IntoIterator<Item = Self>) -> Self {
        let mut out = Vec::new();
        for item in items {
            item.flatten_into(&mut out);
        }
        match out.len() {
            0 => Self::Composition(Vec::new()),
            1 => out.remove(0),
            _ => Self::Composition(out),
        }
    }

    fn flatten_into(self, out: &mut Vec<Self>) {
        match self {
            Self::Composition(items) => {
                for item in items {
                    item.flatten_into(out);
                }
            }
            item => out.push(item),
        }
    }

    fn plan(&self, expanded: bool) -> AccordionMotionPlan {
        let mut plan = AccordionMotionPlan::default();
        self.append_plan(expanded, &mut plan);
        plan.normalize()
    }

    fn append_plan(&self, expanded: bool, plan: &mut AccordionMotionPlan) {
        match self {
            Self::Default | Self::FadeCollapseWithChevron => {
                Self::Collapse.append_plan(expanded, plan);
                Self::Fade.append_plan(expanded, plan);
                Self::Chevron.append_plan(expanded, plan);
            }
            Self::FadeCollapse => {
                Self::Collapse.append_plan(expanded, plan);
                Self::Fade.append_plan(expanded, plan);
            }
            Self::Collapse => {
                push_enter_with_exit(
                    &mut plan.panel_enter,
                    &mut plan.panel_exit,
                    collapse_y_in(180),
                );
                plan.clip_panel = true;
            }
            Self::Fade => {
                push_enter_with_exit(&mut plan.panel_enter, &mut plan.panel_exit, fade_in(140));
            }
            Self::Chevron => plan.indicator.push(
                MotionTrack::composite(
                    MotionPropertyId::Rotation,
                    MotionStartValue::Current,
                    deg(if expanded { 180.0 } else { 0.0 }),
                )
                .transition(MotionTransition::tween(160, MotionEasing::EaseOut)),
            ),
            Self::Composition(items) => {
                for item in items {
                    item.append_plan(expanded, plan);
                }
            }
            Self::None => {}
            Self::Custom {
                panel_enter,
                panel_exit,
                indicator,
                keep_rendered,
            } => {
                plan.panel_enter.extend(panel_enter.clone());
                plan.panel_exit.extend(panel_exit.clone());
                if let Some(indicator) = indicator {
                    plan.indicator.extend(indicator.clone());
                }
                plan.keep_rendered |= *keep_rendered;
            }
        }
    }
}

impl Add for AccordionMotion {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::compose([self, rhs])
    }
}

#[derive(Default)]
struct AccordionMotionPlan {
    panel_enter: Vec<MotionTrack>,
    panel_exit: Vec<MotionTrack>,
    indicator: Vec<MotionTrack>,
    keep_rendered: bool,
    clip_panel: bool,
}

impl AccordionMotionPlan {
    fn normalize(mut self) -> Self {
        if self.panel_exit.is_empty() {
            self.panel_exit = exit_for(&self.panel_enter);
        }
        self.panel_enter = dedupe(self.panel_enter);
        self.panel_exit = dedupe(self.panel_exit);
        self.indicator = dedupe(self.indicator);
        self
    }
}

/// A single collapsible section within an [`Accordion`].
///
/// When `is_expanded` is `true`, the content is visible below the header.
/// The header displays a chevron indicator (triangledown/triangleright) and the title text.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AccordionItem {
    /// Header label for this section.
    pub title: String,
    /// Panel content built while the section is expanded or retained by motion.
    pub content: Widget,
    /// Controlled expanded state.
    pub is_expanded: bool,
    /// Action dispatched when the header requests a state change.
    pub on_toggle: Option<ActionEnvelope>,
}

/// A vertical list of collapsible sections.
///
/// Each [`AccordionItem`] has a clickable header that toggles its content visibility.
/// Items are stacked with zero gap, creating a continuous bordered surface.
#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct Accordion {
    /// Collapsible sections in display order.
    pub items: Vec<AccordionItem>,
    /// Accordion motion. `None` plays the default motion unless the app turns widget motion off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub motion: Option<AccordionMotion>,
}

impl From<Accordion> for Widget {
    fn from(component: Accordion) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        let this = &component;

        let tokens = &view.env().theme.tokens;
        let recipe = view.env().theme.recipe(fission_theme::recipes::Accordion);
        let header_style = recipe.part(fission_theme::recipes::AccordionPart::Header);
        let panel_style = recipe.part(fission_theme::recipes::AccordionPart::Panel);
        let indicator_style = recipe.part(fission_theme::recipes::AccordionPart::Indicator);
        let base_id = fission_core::build::current_widget_id()
            .unwrap_or_else(|| WidgetId::explicit("fission.widgets.accordion.motion"));

        let motion = resolve_motion(
            &this.motion,
            AccordionMotion::Default,
            AccordionMotion::None,
            view.env(),
        );
        let mut children = Vec::new();

        for (index, item) in this.items.iter().enumerate() {
            let panel_id = slot_id(
                WidgetId::derived(base_id.as_u128(), &[index as u32]),
                SLOT_PANEL,
            );
            // The presence wrapper owns panel_id, so the panel's semantics node
            // needs an identity of its own for the header to point at.
            let panel_semantics_id = slot_id(
                WidgetId::derived(base_id.as_u128(), &[index as u32]),
                SLOT_CONTENT,
            );
            let indicator_id = slot_id(
                WidgetId::derived(base_id.as_u128(), &[index as u32]),
                SLOT_INDICATOR,
            );
            let motion_plan = motion.as_ref().map(|motion| motion.plan(item.is_expanded));
            // One square chevron that rotates about its centre. Swapping glyphs while also
            // rotating showed the wrong glyph mid-turn, and a text glyph is not centred in its
            // box, so the turn looked tilted.
            // The recipe's font size described the old text glyph; an icon needs an icon size.
            let indicator_size = indicator_style
                .icon_size
                .unwrap_or(tokens.spacing.m.max(16.0));
            let rotates = motion_plan
                .as_ref()
                .is_some_and(|plan| !plan.indicator.is_empty());
            // Without rotation motion, the expanded state shows the upward chevron directly.
            let chevron = if item.is_expanded && !rotates {
                material::navigation::expand_less::regular()
            } else {
                material::navigation::expand_more::regular()
            };
            let mut indicator: Widget = Icon::svg(chevron)
                .size(indicator_size)
                .color(
                    indicator_style
                        .text_color
                        .unwrap_or(tokens.colors.text_secondary),
                )
                .into();
            if let Some(plan) = &motion_plan {
                if !plan.indicator.is_empty() {
                    indicator = Motion {
                        id: indicator_id,
                        tracks: plan.indicator.clone(),
                        child: indicator,
                        ..Default::default()
                    }
                    .into();
                }
            }
            // Header
            let header_id = slot_id(
                WidgetId::derived(base_id.as_u128(), &[index as u32]),
                SLOT_HEADER,
            );
            children.push(
                SemanticsRegion::new(Button {
                    variant: ButtonVariant::Ghost,
                    content_align: ButtonContentAlign::Start,
                    child: Some(
                        Container::new(HStack {
                            spacing: Some(header_style.gap.unwrap_or(tokens.spacing.s)),
                            children: vec![
                                // Expand icon (chevron)
                                indicator,
                                // Title
                                Text {
                                    content: TextContent::Literal(item.title.clone()),
                                    color: Some(tokens.colors.text_primary),
                                    flex_grow: 1.0,
                                    ..Default::default()
                                }
                                .into(),
                            ],
                        })
                        .padding(header_style.padding_box(tokens.spacing.m, tokens.spacing.m))
                        .bg_fill(
                            header_style
                                .background
                                .clone()
                                .unwrap_or(Fill::Solid(tokens.colors.surface)),
                        )
                        .border(tokens.colors.border, 1.0)
                        // Fill the header row so the bordered surface spans the accordion.
                        .flex_grow(1.0)
                        .into(),
                    ),
                    on_press: item.on_toggle.clone(),
                    ..Default::default()
                })
                // A disclosure header states whether its panel is open and
                // which panel it controls, so a reader knows what pressing it
                // does and can jump to the revealed content.
                .id(header_id)
                .label(item.title.clone())
                .expanded(item.is_expanded)
                .controls(vec![panel_semantics_id])
                .into(),
            );
            // A column does not stretch a bare semantics wrapper, so give the header the full width.
            let header = children.pop().expect("header just pushed");
            children.push(
                Container::new(header)
                    .width_length(fission_core::op::Length::percent(100.0))
                    .into(),
            );

            // Content
            if item.is_expanded || (motion.is_some() && presence_active(panel_id)) {
                let mut panel: Widget = SemanticsRegion::new(
                    Container::new(item.content.clone())
                        .width_length(fission_core::op::Length::percent(100.0))
                        .padding(panel_style.padding_box(tokens.spacing.m, tokens.spacing.m))
                        .bg_fill(
                            panel_style
                                .background
                                .clone()
                                .unwrap_or(Fill::Solid(tokens.colors.background)),
                        )
                        .border(tokens.colors.border, 1.0),
                )
                .id(panel_semantics_id)
                .role(Role::Group)
                .labelled_by(vec![header_id])
                .into();
                if let Some(plan) = &motion_plan {
                    panel = Presence {
                        id: panel_id,
                        visible: item.is_expanded,
                        enter: plan.panel_enter.clone(),
                        exit: plan.panel_exit.clone(),
                        keep_rendered: plan.keep_rendered,
                        clip_to_bounds: plan.clip_panel,
                        child: panel,
                        ..Default::default()
                    }
                    .into();
                }
                // Like the header, the panel spans the accordion; a column does not stretch it.
                children.push(
                    Container::new(panel)
                        .width_length(fission_core::op::Length::percent(100.0))
                        .into(),
                );
            }
        }

        VStack {
            spacing: Some(0.0), // No gap between items
            children,
        }
        .into()
    }
}
