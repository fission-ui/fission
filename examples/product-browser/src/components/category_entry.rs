use fission::prelude::*;

#[derive(Clone, Debug)]
pub struct CategoryEntry {
    pub label: String,
    pub action: ActionEnvelope,
    pub selected: bool,
    pub identifier: String,
}

impl From<CategoryEntry> for Widget {
    fn from(component: CategoryEntry) -> Self {
        let identifier = component.identifier;

        Button {
            id: Some(WidgetId::explicit(&identifier)),
            on_press: Some(component.action),
            variant: if component.selected {
                ButtonVariant::Filled
            } else {
                ButtonVariant::Ghost
            },
            content_align: ButtonContentAlign::Start,
            child: Some(Text::new(component.label).max_lines(1).into()),
            ..Default::default()
        }
        .semantics_identifier(identifier)
        .into()
    }
}
