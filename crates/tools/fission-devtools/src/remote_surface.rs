use std::fmt;
use std::sync::Arc;

use fission_core::internal::{CustomRender, InternalLowerer, InternalLoweringCx};
use fission_core::{Widget, WidgetId};

use crate::AppFrame;

/// Imports one validated application frame into a resident Fission host tree.
///
/// The host continues to own rendering, layout, input, focus, scroll and text
/// editing state. The application generation supplies only ordinary Fission
/// Core IR with its stable widget identities.
#[derive(Clone)]
pub struct RemoteAppSurface {
    frame: Arc<AppFrame>,
}

impl RemoteAppSurface {
    pub fn new(frame: Arc<AppFrame>) -> Self {
        Self { frame }
    }
}

impl From<RemoteAppSurface> for Widget {
    fn from(surface: RemoteAppSurface) -> Self {
        CustomRender::new(
            "FissionDeveloperAppSurface",
            Arc::new(RemoteIrLowerer {
                frame: surface.frame,
            }),
        )
        .into()
    }
}

struct RemoteIrLowerer {
    frame: Arc<AppFrame>,
}

impl fmt::Debug for RemoteIrLowerer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RemoteIrLowerer")
            .field("generation", &self.frame.generation)
            .field("nodes", &self.frame.ir.nodes.len())
            .finish()
    }
}

impl InternalLowerer for RemoteIrLowerer {
    fn lower_dyn(&self, cx: &mut InternalLoweringCx) -> WidgetId {
        let root = self
            .frame
            .ir
            .root
            .expect("validated Fission Developer frame must contain a root");
        for (id, node) in &self.frame.ir.nodes {
            assert!(
                !cx.ir.nodes.contains_key(id),
                "Fission Developer host/app WidgetId collision for {id}"
            );
            cx.ir.nodes.insert(*id, node.clone());
        }
        root
    }
}
