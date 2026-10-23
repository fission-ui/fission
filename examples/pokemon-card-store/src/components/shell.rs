use crate::app::StoreState;
use crate::components::layout::STORE_MIN_HEIGHT;
use crate::components::store_footer::StoreFooter;
use crate::components::store_nav::StoreNav;
use fission::prelude::*;

fission_component! {
    #[derive(Clone)] pub StoreShell<StoreState> { child: Widget }
    |component, _state, cx| { let tokens = &cx.view.env().theme.tokens; } =>
        Container(Column(gap: tokens.spacing.l, children: [StoreNav(), component.child.clone(), StoreFooter()]))
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
        .bg(tokens.colors.background)
}
