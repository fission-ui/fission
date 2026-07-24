use fission::prelude::*;

pub struct NotFoundPage;

impl From<NotFoundPage> for Widget {
    fn from(_page: NotFoundPage) -> Self {
        Text::new("404: animation gallery page not found").into()
    }
}
