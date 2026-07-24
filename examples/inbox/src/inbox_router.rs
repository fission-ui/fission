use crate::components::{EmailDetail, EmailList};
use crate::model::InboxState;
use fission::core::ui::{Text, Widget};
use fission::widgets::{Route, Router};
use std::sync::Arc;

pub(crate) struct InboxRouter;

impl From<InboxRouter> for Widget {
    fn from(_router: InboxRouter) -> Self {
        let (_, view) = fission::build::current::<InboxState>();

        Router::<InboxState> {
            current_path: view.state().current_path.clone(),
            routes: vec![
                Route {
                    path: "/inbox".into(),
                    builder: Arc::new(|_, _, _| {
                        EmailList {
                            folder: "Inbox".into(),
                        }
                        .into()
                    }),
                },
                Route {
                    path: "/:folder/:id".into(),
                    builder: Arc::new(|_, _, params| {
                        EmailDetail {
                            folder: params["folder"].clone(),
                            id: params["id"].parse().unwrap_or(0),
                        }
                        .into()
                    }),
                },
                Route {
                    path: "/:folder".into(),
                    builder: Arc::new(|_, _, params| {
                        EmailList {
                            folder: params["folder"].clone(),
                        }
                        .into()
                    }),
                },
            ],
            not_found: Some(Arc::new(|_, _, _| Text::new("Folder not found").into())),
        }
        .into()
    }
}
