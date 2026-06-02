use fission_core::internal::{
    InternalIrBuilder, InternalLowerer, InternalLoweringCx, InternalRenderNode,
};
use fission_core::{Widget, WidgetId};
use fission_ir::{EmbedKind, LayoutOp, Op};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WebView {
    pub id: WidgetId,
    pub url: String,
    pub user_agent: Option<String>,
    pub width: Option<f32>,
    pub height: Option<f32>,
}

impl From<WebView> for Widget {
    fn from(component: WebView) -> Self {
        let (ctx, _) = fission_core::build::current::<()>();
        let mut component = component;
        if let Some(id) = fission_core::build::current_widget_id() {
            component.id = id;
        }
        let this = &component;

        ctx.register_web_view(fission_core::registry::WebRegistration {
            node_id: this.id,
            url: this.url.clone(),
            user_agent: this.user_agent.clone(),
        });

        fission_core::internal::custom_render_widget(InternalRenderNode {
            debug_tag: "WebView".into(),
            lowerer: Some(std::sync::Arc::new(WebViewLowerer {
                id: this.id,
                url: this.url.clone(),
                width: this.width,
                height: this.height,
            })),
            render_object: None,
        })
    }
}

#[derive(Debug)]
struct WebViewLowerer {
    id: WidgetId,
    url: String,
    width: Option<f32>,
    height: Option<f32>,
}

impl InternalLowerer for WebViewLowerer {
    fn lower_dyn(&self, cx: &mut InternalLoweringCx) -> WidgetId {
        let id = cx.widget_node_id(self.id);

        let builder = InternalIrBuilder::new(
            id,
            Op::Layout(LayoutOp::Embed {
                kind: EmbedKind::Web,
                widget_id: self.id,
                width: self.width,
                height: self.height,
            }),
        );

        builder.build(cx)
    }

    fn stable_key(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.id.hash(&mut h);
        self.url.hash(&mut h);
        h.finish()
    }
}
