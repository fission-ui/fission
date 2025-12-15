use crate::{
    env::VideoState,
    registry::{AnimationPropertyId, VideoRegistration},
    ui::{Button, Column, Image, Node, Row, Scroll, Text, Video},
    AppState, BuildCtx, Env, RuntimeState,
};
use fission_i18n::I18nRegistry;
use fission_ir::WidgetNodeId;
use fission_theme::Theme;

pub struct View<'a, S: AppState> {
    pub state: &'a S,
    pub runtime: &'a RuntimeState,
    pub env: &'a Env,
}

impl<'a, S: AppState> View<'a, S> {
    pub fn new(state: &'a S, runtime: &'a RuntimeState, env: &'a Env) -> Self {
        Self {
            state,
            runtime,
            env,
        }
    }

    pub fn theme(&self) -> &Theme {
        &self.env.theme
    }
    pub fn i18n(&self) -> &I18nRegistry {
        &self.env.i18n
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

// Implement Widget for Primitives
impl<S: AppState> Widget<S> for Row {
    fn build(&self, _ctx: &mut BuildCtx<S>, _view: &View<S>) -> Node {
        Node::Row(self.clone())
    }
}

impl<S: AppState> Widget<S> for Column {
    fn build(&self, _ctx: &mut BuildCtx<S>, _view: &View<S>) -> Node {
        Node::Column(self.clone())
    }
}

impl<S: AppState> Widget<S> for Text {
    fn build(&self, _ctx: &mut BuildCtx<S>, _view: &View<S>) -> Node {
        Node::Text(self.clone())
    }
}

impl<S: AppState> Widget<S> for Button {
    fn build(&self, _ctx: &mut BuildCtx<S>, _view: &View<S>) -> Node {
        Node::Button(self.clone())
    }
}

impl<S: AppState> Widget<S> for Scroll {
    fn build(&self, _ctx: &mut BuildCtx<S>, _view: &View<S>) -> Node {
        Node::Scroll(self.clone())
    }
}

impl<S: AppState> Widget<S> for Image {
    fn build(&self, _ctx: &mut BuildCtx<S>, _view: &View<S>) -> Node {
        Node::Image(self.clone())
    }
}

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
