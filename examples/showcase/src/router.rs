use crate::components::AppHeader;
use crate::pages::{DiscoverPage, ExampleDetailPage, NotFoundPage};
use crate::state::ShowcaseState;
use fission::prelude::*;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub(crate) struct ShowcaseRouter;

impl From<ShowcaseRouter> for Widget {
    fn from(_component: ShowcaseRouter) -> Self {
        let (_ctx, view) = fission::build::current::<ShowcaseState>();
        let router = Router::<ShowcaseState> {
            current_path: view.state().current_path.clone(),
            routes: vec![
                Route {
                    path: "/".into(),
                    builder: Arc::new(|_, _, _| DiscoverPage.into()),
                },
                Route {
                    path: "/examples/:slug".into(),
                    builder: Arc::new(|_, _, params| {
                        ExampleDetailPage {
                            slug: params.get("slug").cloned().unwrap_or_default(),
                        }
                        .into()
                    }),
                },
            ],
            not_found: Some(Arc::new(|_, _, _| NotFoundPage.into())),
        };

        Column {
            children: widgets![
                AppHeader,
                Container::new(router)
                    .flex_grow(1.0)
                    .min_height(0.0)
                    .id(WidgetId::explicit("showcase.route.content")),
            ],
            gap: Some(0.0),
            ..Default::default()
        }
        .into()
    }
}
