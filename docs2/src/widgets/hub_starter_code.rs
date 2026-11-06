use crate::state::DocsState;
use fission::prelude::*;

fission_component! {
    pub HubStarterCode<DocsState> {}
    view(_panel, _state, cx) {
        let commands = "fission init my-app\ncd my-app\nfission run";
        let highlighted_commands = format!("```sh\n{commands}\n```");
        let tokens = &cx.view.env().theme.tokens;
    } =>
    SemanticsRegion(
        identifier: Some("hub-starter-code".to_string()),
        child: Column(children: [
            SemanticsRegion(
                Text(cx.view.env().tr("hub.target.shell"))
                    .weight(tokens.typography.font_weight_semibold),
                identifier: Some("hub-starter-label".to_string()),
            ),
            MarkdownContent::new(highlighted_commands),
        ]),
    )
}
