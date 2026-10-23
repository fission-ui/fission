use crate::components::AppHeader;
use crate::pages::{DiscoverPage, ExampleDetailPage, NotFoundPage};
use crate::state::ShowcaseState;
use fission::prelude::*;
use std::sync::Arc;

fission_component! {
    #[derive(Clone, Debug)] pub(crate) ShowcaseRouter<ShowcaseState> {}
    |_component, state, _cx| {
        let router = Router::<ShowcaseState> {
            current_path: state.current_path.clone(),
            routes: vec![
                Route {
                    path: "/".into(),
                    builder: Arc::new(|_, _, _| DiscoverPage::new().into()),
                },
                Route {
                    path: "/examples/:slug".into(),
                    builder: Arc::new(|_, _, params| {
                        ExampleDetailPage::new(params.get("slug").cloned().unwrap_or_default()).into()
                    }),
                },
            ],
            not_found: Some(Arc::new(|_, _, _| NotFoundPage::new().into())),
        };
    } =>
        Column(
            children: [
                AppHeader(),
                Container(router)
                    .flex_grow(1.0)
                    .min_height(0.0)
                    .id(WidgetId::explicit("showcase.route.content")),
            ],
            gap: 0.0,
        )
}
