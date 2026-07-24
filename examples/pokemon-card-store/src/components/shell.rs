use crate::app::StoreState;
use crate::components::layout::STORE_MIN_HEIGHT;
use crate::components::palette::CANVAS;
use crate::components::store_footer::StoreFooter;
use crate::components::store_nav::StoreNav;
use fission::prelude::*;

#[derive(Clone)]
pub struct StoreShell {
    pub child: Widget,
}

impl From<StoreShell> for Widget {
    fn from(component: StoreShell) -> Self {
        let (_ctx, view) = fission::build::current::<StoreState>();
        let tokens = &view.env().theme.tokens;
        Container::new(Column {
            gap: Some(tokens.spacing.l),
            children: widgets![StoreNav, component.child, StoreFooter],
            ..Default::default()
        })
        .min_height_length(Length::max(vec![
            Length::vh(100.0),
            Length::points(STORE_MIN_HEIGHT),
        ]))
        .padding_lengths([
            Length::points(tokens.spacing.xl),
            Length::points(tokens.spacing.xl),
            Length::points(tokens.spacing.l),
            Length::points(tokens.spacing.xl),
        ])
        .bg(CANVAS)
        .into()
    }
}
