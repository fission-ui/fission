use crate::Icon;
use fission_core::ui::{Button, ButtonContentAlign, ButtonVariant, Container, Row, Text, Widget};
use fission_core::ActionEnvelope;
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

impl From<Breadcrumb> for Widget {
    fn from(component: Breadcrumb) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        let this = &component;

        let tokens = &view.env().theme.tokens;
        let mut children = Vec::new();

        for (i, item) in this.items.iter().enumerate() {
            let is_last = i == this.items.len() - 1;

            if i > 0 {
                children.push(
                    Icon::svg(material::navigation::chevron_right::regular())
                        .size(16.0)
                        .color(tokens.colors.text_secondary)
                        .into(),
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
                        .into(),
                );
            } else {
                let button: Widget = Button {
                    variant: ButtonVariant::Ghost,
                    content_align: ButtonContentAlign::Start,
                    child: Some(
                        Text::new(item.label.clone())
                            .color(tokens.colors.text_secondary)
                            .flex_shrink(0.0)
                            .into(),
                    ),
                    on_press: item.on_click.clone(),
                    ..Default::default()
                }
                .into();

                children.push(Container::new(button).flex_shrink(0.0).into());
            }
        }

        Row {
            gap: Some(8.0),
            align_items: fission_ir::op::AlignItems::Center,
            children,
            ..Default::default()
        }
        .into()
    }
}
