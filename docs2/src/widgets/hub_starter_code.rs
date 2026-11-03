use crate::state::DocsState;
use fission::prelude::*;

fission_component! {
    pub HubStarterCode<DocsState> {}
    view(_panel, _state, cx) {
        let commands = [
            "fission init my-app\ncd my-app\nfission run",
        ];
    } =>
    SemanticsRegion(
        identifier: Some("hub-starter-code".to_string()),
        child: Column(children: [
            Row(children: [
                SemanticsRegion(Text(cx.view.env().tr("hub.target.shell")), identifier: Some("hub-target-tab-selected".to_string())),
                SemanticsRegion(Text(cx.view.env().tr("hub.target.desktop")), identifier: Some("hub-target-tab".to_string())),
                SemanticsRegion(Text(cx.view.env().tr("hub.target.web")), identifier: Some("hub-target-tab".to_string())),
                SemanticsRegion(Text(cx.view.env().tr("hub.target.mobile")), identifier: Some("hub-target-tab".to_string())),
            ]),
            Code(text: commands[0].to_string()),
        ]),
    )
}
