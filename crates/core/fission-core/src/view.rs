use crate::{
    env::VideoState,
    registry::{AnimationPropertyId, VideoRegistration},
    ui::{Align, Button, Checkbox, Column, Container, Grid, GridItem, Image, LazyColumn, Node, Overlay, Positioned, Radio, Row, Scroll, Slider, Spacer, Switch, Text, TextInput, Video, ZStack},
    AppState, BuildCtx, Env, RuntimeState, LayoutSnapshot, LayoutRect, LayoutSize,
};
use fission_i18n::I18nRegistry;
use fission_ir::{NodeId, WidgetNodeId};
use fission_layout::BoxConstraints;
use fission_theme::Theme;

pub struct View<'a, S: AppState> {
    pub state: &'a S,
    pub runtime: &'a RuntimeState,
    pub env: &'a Env,
    pub layout: Option<&'a LayoutSnapshot>,
}

impl<'a, S: AppState> View<'a, S> {
    pub fn new(state: &'a S, runtime: &'a RuntimeState, env: &'a Env, layout: Option<&'a LayoutSnapshot>) -> Self {
        Self {
            state,
            runtime,
            env,
            layout,
        }
    }

    pub fn theme(&self) -> &Theme {
        &self.env.theme
    }
    pub fn i18n(&self) -> &I18nRegistry {
        &self.env.i18n
    }

    pub fn get_rect(&self, id: WidgetNodeId) -> Option<LayoutRect> {
        let node_id: NodeId = id.into();
        self.layout.and_then(|l| l.get_node_rect(node_id))
    }

    pub fn get_constraints(&self, id: WidgetNodeId) -> Option<BoxConstraints> {
        let node_id: NodeId = id.into();
        self.layout.and_then(|l| l.get_node_constraints(node_id))
    }

    pub fn viewport_size(&self) -> LayoutSize {
        self.env.viewport_size
    }

    pub fn select<T: Selector<S>>(&self) -> T::Output {
        T::select(self)
    }

    pub fn animation_value(&self, widget_id: WidgetNodeId, property: &AnimationPropertyId) -> f32 {
        self.runtime
            .animation
            .values
            .get(&(widget_id, property.clone()))
            .copied()
            .unwrap_or_else(|| property.default_value())
    }

    pub fn video_state(&self, widget_id: WidgetNodeId) -> Option<&VideoState> {
        self.runtime.video.states.get(&widget_id)
    }
}

pub trait Selector<S: AppState> {
    type Output;
    fn select(view: &View<S>) -> Self::Output;
}

pub trait Widget<S: AppState> {
    fn build(&self, ctx: &mut BuildCtx<S>, view: &View<S>) -> Node;
}

// Implement Widget for Node (identity)
impl<S: AppState> Widget<S> for Node {
    fn build(&self, _ctx: &mut BuildCtx<S>, _view: &View<S>) -> Node {
        self.clone()
    }
}

macro_rules! impl_widget_for_primitive {
    ($t:ty, $v:ident) => {
        impl<S: AppState> Widget<S> for $t {
            fn build(&self, _ctx: &mut BuildCtx<S>, _view: &View<S>) -> Node {
                Node::$v(self.clone())
            }
        }
    };
}

impl_widget_for_primitive!(Row, Row);
impl_widget_for_primitive!(Column, Column);
impl_widget_for_primitive!(Align, Align);
impl_widget_for_primitive!(Text, Text);
impl_widget_for_primitive!(Button, Button);
impl_widget_for_primitive!(TextInput, TextInput);
impl_widget_for_primitive!(Scroll, Scroll);
impl_widget_for_primitive!(Image, Image);
impl_widget_for_primitive!(ZStack, ZStack);
impl_widget_for_primitive!(Overlay, Overlay);
impl_widget_for_primitive!(Container, Container);
impl_widget_for_primitive!(Grid, Grid);
impl_widget_for_primitive!(GridItem, GridItem);
impl_widget_for_primitive!(Checkbox, Checkbox);
impl_widget_for_primitive!(Switch, Switch);
impl_widget_for_primitive!(Radio, Radio);
impl_widget_for_primitive!(Positioned, Positioned);
impl_widget_for_primitive!(Spacer, Spacer);
impl_widget_for_primitive!(Slider, Slider);
impl_widget_for_primitive!(LazyColumn, LazyColumn);

impl<S: AppState> Widget<S> for Video {
    fn build(&self, ctx: &mut BuildCtx<S>, _view: &View<S>) -> Node {
        let mut video = self.clone();
        let id = video
            .id
            .unwrap_or_else(|| WidgetNodeId::explicit(&video.source));
        video.id = Some(id);

        ctx.register_video(VideoRegistration {
            node_id: id,
            source: video.source.clone(),
            autoplay: video.autoplay,
            loop_playback: video.loop_playback,
        });

        Node::Video(video)
    }
}
