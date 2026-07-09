use super::boards::PublishBoardCanvas;
use super::chrome::{PublishFooter, PublishHeader};
use super::*;

#[derive(Clone)]
pub struct PublishApp;

impl From<PublishApp> for Widget {
    fn from(_component: PublishApp) -> Widget {
        let (_ctx, view) = fission::build::current::<PublishUiState>();
        let size = view.env().viewport_size;
        let palette = PublishPalette::for_mode(view.state().theme_mode);
        let layout = PublishLayout::from_viewport(size);
        let body = Column {
            gap: Some(layout.gap),
            children: widgets![
                PublishHeader { layout },
                PublishBoardCanvas { layout },
                PublishFooter { layout },
            ],
            ..Default::default()
        };
        Container::new(body)
            .width(size.width)
            .height(size.height)
            .padding([
                layout.root_padding,
                layout.root_padding,
                layout.root_padding,
                layout.root_padding,
            ])
            .bg(palette.background)
            .into()
    }
}
