use crate::{AppState, RuntimeState, Env, BuildCtx, ui::{Node, Row, Column, Text, Button}};
use fission_theme::Theme;
use fission_i18n::I18nRegistry;

pub struct View<'a, S: AppState> {
    pub state: &'a S,
    pub runtime: &'a RuntimeState,
    pub env: &'a Env,
}

impl<'a, S: AppState> View<'a, S> {
    pub fn new(state: &'a S, runtime: &'a RuntimeState, env: &'a Env) -> Self {
        Self { state, runtime, env }
    }
    
    pub fn theme(&self) -> &Theme { &self.env.theme }
    pub fn i18n(&self) -> &I18nRegistry { &self.env.i18n }

    pub fn select<T: Selector<S>>(&self) -> T::Output {
        T::select(self)
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
