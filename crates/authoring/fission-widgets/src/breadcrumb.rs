use crate::Icon;
use fission_core::ui::{Button, ButtonContentAlign, ButtonVariant, Container, Row, Text};
use fission_core::{ActionEnvelope, BuildCtx, View, Widget};
use fission_icons::material;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BreadcrumbItem {
    pub label: String,
    pub on_click: Option<ActionEnvelope>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Breadcrumb {
    pub items: Vec<BreadcrumbItem>,
}

impl<S: fission_core::AppState> Widget<S> for Breadcrumb {
    fn build(&self, _ctx: &mut BuildCtx<S>, view: &View<S>) -> impl fission_core::IntoWidget<S> {
        fission_core::AnyWidget::from_node({
            let tokens = &view.env.theme.tokens;
            let mut children = Vec::new();

            for (i, item) in self.items.iter().enumerate() {
                let is_last = i == self.items.len() - 1;

                if i > 0 {
                    children.push(
                        Icon::svg(material::navigation::chevron_right::regular())
                            .size(16.0)
                            .color(tokens.colors.text_secondary)
                            .into_node(),
                    );
                }

                if is_last || item.on_click.is_none() {
                    let mut text = Text::new(item.label.clone())
                        .color(if is_last {
                            tokens.colors.text_primary
                        } else {
                            tokens.colors.text_secondary
                        })
                        .flex_shrink(0.0);
                    if is_last {
                        text = text.flex_grow(1.0);
                    }
                    children.push(
                        text
                            // .weight(if is_last { Bold } else { Normal })
                            .into_node(),
                    );
                } else {
                    let button = Button {
                        variant: ButtonVariant::Ghost,
                        content_align: ButtonContentAlign::Start,
                        child: Some(Box::new(
                            Text::new(item.label.clone())
                                .color(tokens.colors.text_secondary)
                                .flex_shrink(0.0)
                                .into_node(),
                        )),
                        on_press: item.on_click.clone(),
                        ..Default::default()
                    }
                    .into_node();

                    children.push(Container::new(button).flex_shrink(0.0).into_node());
                }
            }

            Row {
                gap: Some(8.0),
                align_items: fission_ir::op::AlignItems::Center,
                children,
                ..Default::default()
            }
            .into_node()
        })
    }
}
