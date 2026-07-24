use crate::state::AnimationGalleryState;
use fission::build::BuildCtxHandle;
use fission::prelude::*;
use serde::Serialize;

pub struct SmallButton<'a, A>
where
    A: Action + Serialize + Clone + 'static,
{
    pub ctx: &'a BuildCtxHandle<AnimationGalleryState>,
    pub label: &'a str,
    pub action: A,
    pub reducer: Handler<AnimationGalleryState, A>,
}

impl<A> From<SmallButton<'_, A>> for Widget
where
    A: Action + Serialize + Clone + 'static,
{
    fn from(button: SmallButton<'_, A>) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();
        Button {
            variant: ButtonVariant::Outline,
            child: Some(
                Text::new(button.label)
                    .size(view.env().theme.tokens.typography.font_size_xs)
                    .into(),
            ),
            on_press: Some(button.ctx.bind(button.action, button.reducer)),
            ..Default::default()
        }
        .into()
    }
}
