use crate::motion_support::{
    dedupe, exit_for, fade_in, push_enter_with_exit, slide_x_in, slot_id, SLOT_CONTENT,
    SLOT_INDICATOR,
};
use crate::stack::{HStack, VStack};
use fission_core::motion::{follow_x_and_width, Motion, MotionTrack, Presence};
use fission_core::ui::{
    Button, ButtonVariant, ComponentSize, ComponentState, Container, Text, Widget,
};
use fission_core::{ActionEnvelope, WidgetId, WidgetIdExt};
use serde::{Deserialize, Serialize};
use std::ops::Add;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
/// Optional motion presets owned by [`Tabs`].
///
/// Tabs render immediately unless [`Tabs::motion`] is set. Presets lower to
/// motion tracks for the stable `indicator` slot and presence tracks for the
/// active `content` slot.
///
/// ```rust,ignore
/// let motion = Some(TabsMotion::Indicator + TabsMotion::SlideContent);
/// ```
pub enum TabsMotion {
    /// Curated default: indicator plus fading content.
    Default,
    /// Animate the active indicator toward the active tab trigger.
    Indicator,
    /// Fade tab content.
    FadeContent,
    /// Slide tab content.
    SlideContent,
    /// Compound indicator plus fade content preset.
    IndicatorAndFade,
    /// Compound indicator plus slide content preset.
    IndicatorAndSlide,
    /// Ordered composition of tabs motion atoms.
    Composition(Vec<TabsMotion>),
    /// Caller-provided tracks for tabs stable slots.
    Custom {
        /// Optional indicator tracks.
        indicator: Option<Vec<MotionTrack>>,
        /// Content enter tracks.
        content_enter: Vec<MotionTrack>,
        /// Content exit tracks.
        content_exit: Vec<MotionTrack>,
        /// Whether content remains rendered after exit completes.
        keep_rendered: bool,
    },
}

impl TabsMotion {
    /// Flattens and normalizes an ordered tabs-motion composition.
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

    fn plan(&self, active_trigger: WidgetId) -> TabsMotionPlan {
        let mut plan = TabsMotionPlan::default();
        self.append_plan(active_trigger, &mut plan);
        plan.normalize()
    }

    fn append_plan(&self, active_trigger: WidgetId, plan: &mut TabsMotionPlan) {
        match self {
            Self::Default | Self::IndicatorAndFade => {
                Self::Indicator.append_plan(active_trigger, plan);
                Self::FadeContent.append_plan(active_trigger, plan);
            }
            Self::IndicatorAndSlide => {
                Self::Indicator.append_plan(active_trigger, plan);
                Self::SlideContent.append_plan(active_trigger, plan);
            }
            Self::Indicator => plan.indicator.extend(follow_x_and_width(active_trigger)),
            Self::FadeContent => {
                push_enter_with_exit(
                    &mut plan.content_enter,
                    &mut plan.content_exit,
                    fade_in(140),
                );
            }
            Self::SlideContent => {
                push_enter_with_exit(
                    &mut plan.content_enter,
                    &mut plan.content_exit,
                    slide_x_in(12.0, 160),
                );
            }
            Self::Composition(items) => {
                for item in items {
                    item.append_plan(active_trigger, plan);
                }
            }
            Self::Custom {
                indicator,
                content_enter,
                content_exit,
                keep_rendered,
            } => {
                if let Some(indicator) = indicator {
                    plan.indicator.extend(indicator.clone());
                }
                plan.content_enter.extend(content_enter.clone());
                plan.content_exit.extend(content_exit.clone());
                plan.keep_rendered |= *keep_rendered;
            }
        }
    }
}

impl Add for TabsMotion {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::compose([self, rhs])
    }
}

#[derive(Default)]
struct TabsMotionPlan {
    indicator: Vec<MotionTrack>,
    content_enter: Vec<MotionTrack>,
    content_exit: Vec<MotionTrack>,
    keep_rendered: bool,
}

impl TabsMotionPlan {
    fn normalize(mut self) -> Self {
        if self.content_exit.is_empty() {
            self.content_exit = exit_for(&self.content_enter);
        }
        self.indicator = dedupe(self.indicator);
        self.content_enter = dedupe(self.content_enter);
        self.content_exit = dedupe(self.content_exit);
        self
    }
}

/// A single tab definition containing a title, content node, and selection action.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TabItem {
    pub title: String,
    pub content: Widget,
    pub on_press: Option<ActionEnvelope>,
}

/// A tab bar with an active indicator and swappable content area.
///
/// The tab bar displays a horizontal row of tab buttons. The active tab shows
/// a colored indicator bar below its label. The content area below the tab bar
/// displays the `content` node of the tab at `active_index`.
///
/// # Example
///
/// ```rust,ignore
/// Tabs {
///     active_index: 0,
///     items: vec![
///         TabItem { title: "General".into(), content: general_view, on_press: Some(tab0) },
///         TabItem { title: "Advanced".into(), content: advanced_view, on_press: Some(tab1) },
///     ],
/// }
/// ```
#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct Tabs {
    pub active_index: usize,
    pub items: Vec<TabItem>,
    pub size: ComponentSize,
    /// Optional explicit tabs motion. `None` emits no tabs-owned motion declarations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub motion: Option<TabsMotion>,
}

impl From<Tabs> for Widget {
    fn from(component: Tabs) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        let this = &component;

        let theme = &view.env().theme.components.tabs;
        let mut tab_buttons = vec![];
        let base_id = fission_core::build::current_widget_id()
            .unwrap_or_else(|| WidgetId::explicit("fission.widgets.tabs.motion"));
        let active_trigger = WidgetId::derived(base_id.as_u128(), &[this.active_index as u32, 0]);
        let motion_plan = this
            .motion
            .as_ref()
            .map(|motion| motion.plan(active_trigger));

        for (i, item) in this.items.iter().enumerate() {
            let trigger_id = WidgetId::derived(base_id.as_u128(), &[i as u32, 0]);
            let is_active = i == this.active_index;
            let state = if is_active {
                ComponentState::Active
            } else {
                ComponentState::Default
            };
            let style = theme.resolve_tab(this.size, state);
            let color = style.text_color.unwrap_or(if is_active {
                theme.active_color
            } else {
                theme.inactive_color
            });
            let border = style.border.clone();

            let mut indicator: Widget = if is_active {
                Container::new(fission_core::ui::widgets::spacer::Spacer::default())
                    .height(
                        border
                            .as_ref()
                            .map(|border| border.width)
                            .unwrap_or(theme.indicator_height),
                    )
                    .bg(match border.map(|border| border.fill) {
                        Some(fission_core::op::Fill::Solid(color)) => color,
                        _ => theme.active_color,
                    })
                    .into()
            } else {
                fission_core::ui::widgets::spacer::Spacer::default().into()
            };
            if is_active {
                if let Some(plan) = &motion_plan {
                    if !plan.indicator.is_empty() {
                        indicator = Motion {
                            id: slot_id(base_id, SLOT_INDICATOR),
                            tracks: plan.indicator.clone(),
                            child: indicator,
                            ..Default::default()
                        }
                        .into();
                    }
                }
            }

            let tab_button: Widget = VStack {
                spacing: Some(0.0),
                children: vec![
                    Button {
                        variant: ButtonVariant::Ghost,
                        child: Some(
                            Text::new(item.title.clone())
                                .size(style.font_size.unwrap_or(14.0))
                                .weight(style.font_weight.unwrap_or(400))
                                .color(color)
                                .into(),
                        ),
                        on_press: item.on_press.clone(),
                        height: style.height.or(Some(38.0)),
                        padding: Some([
                            10.0,
                            10.0,
                            style.padding_y.unwrap_or(0.0),
                            style.padding_y.unwrap_or(0.0),
                        ]),
                        ..Default::default()
                    }
                    .id(trigger_id),
                    indicator,
                ],
            }
            .into();

            tab_buttons.push(Container::new(tab_button).padding_all(2.0).into());
        }

        let tab_bar = Container::new(HStack {
            spacing: Some(14.0),
            children: tab_buttons,
        })
        .bg_fill(
            theme
                .track_style
                .background
                .clone()
                .unwrap_or(fission_core::op::Fill::Solid(theme.background)),
        )
        .border(
            theme
                .track_style
                .border
                .as_ref()
                .and_then(|border| match &border.fill {
                    fission_core::op::Fill::Solid(color) => Some(*color),
                    _ => None,
                })
                .unwrap_or(theme.divider_color),
            theme
                .track_style
                .border
                .as_ref()
                .map(|border| border.width)
                .unwrap_or(1.0),
        )
        .padding_all(2.0)
        .into();

        let mut active_content = if let Some(tab) = this.items.get(this.active_index) {
            tab.content.clone()
        } else {
            fission_core::ui::widgets::spacer::Spacer::default().into()
        };
        if let Some(plan) = &motion_plan {
            active_content = Presence {
                id: WidgetId::derived(
                    slot_id(base_id, SLOT_CONTENT).as_u128(),
                    &[this.active_index as u32],
                ),
                visible: true,
                enter: plan.content_enter.clone(),
                exit: plan.content_exit.clone(),
                keep_rendered: plan.keep_rendered,
                child: active_content,
                ..Default::default()
            }
            .into();
        }

        VStack {
            spacing: Some(12.0),
            children: vec![tab_bar, active_content],
        }
        .into()
    }
}
