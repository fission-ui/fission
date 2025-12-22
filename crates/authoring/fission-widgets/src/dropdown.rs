use fission::prelude::*;

#[derive(Default)]
pub struct DropDown {
    pub is_open: bool,
    pub on_toggle: Option<Action>,
    pub options: Vec<String>,
    pub on_select: Option<Action<(String)>>,
    pub selected: Option<String>,
}

impl<S: 'static> Widget<S> for DropDown {
    fn build(&self, ctx: &mut BuildCtx<S>, _view: &View<S>) -> Node {
        let mut children = vec![];

        let button_text = self.selected.as_deref().unwrap_or("Select an option");

        children.push(
            Button::new(button_text)
                .on_press(self.on_toggle)
                .into(),
        );

        if self.is_open {
            let mut options_list = vec![];
            for option in &self.options {
                options_list.push(
                    Button::new(option.clone())
                        .on_press(self.on_select.map(|a| a.with(option.clone())))
                        .into(),
                );
            }
            children.push(Column::new(options_list).into());
        }

        Column::new(children).into()
    }
}
