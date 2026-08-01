use crate::state::ShowcaseState;
use fission::prelude::*;
use fission::widgets::Center;

#[derive(Clone, Debug)]
pub(crate) struct NotFoundPage;

impl From<NotFoundPage> for Widget {
    fn from(_component: NotFoundPage) -> Self {
        let (_ctx, view) = fission::build::current::<ShowcaseState>();
        let tokens = &view.env().theme.tokens;
        Center {
            child: Text::new("Example route not found")
                .size(tokens.typography.heading1_size)
                .family(tokens.typography.font_family_serif.clone())
                .weight(tokens.typography.font_weight_bold)
                .color(tokens.colors.heading)
                .into(),
        }
        .into()
    }
}
