use fission::op::ImageFit;
use fission::prelude::*;

fission_component! {
/// The thread's attachments while they are scanned.
    pub(super) Attachments<()> |_attachments, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
    Container(
        VStack(
            spacing: tokens.spacing.s,
            children: [
                Text(TextContent::Key("email.attachments".into()))
                    .size(tokens.typography.font_size_lg),
                HStack(
                    spacing: tokens.spacing.s,
                    children: [
                        Spinner(
                            id: WidgetId::explicit("attachments_spinner"),
                            motion: SpinnerMotion::Default,
                        ),
                        Text(TextContent::Key("email.scanning_attachments".into()))
                            .size(tokens.typography.font_size_xs)
                            .color(tokens.colors.text_secondary),
                    ],
                ),
                SimpleGrid(
                    min_child_width: 120.0,
                    gap: tokens.spacing.s,
                    children: [
                        AspectRatio(
                            ratio: 4.0 / 3.0,
                            child: Image::network("https://picsum.photos/200/150")
                                .fit(ImageFit::Cover),
                        ),
                        AspectRatio(
                            ratio: 4.0 / 3.0,
                            child: Image::network("https://picsum.photos/201/150")
                                .fit(ImageFit::Cover),
                        ),
                        AspectRatio(
                            ratio: 16.0 / 9.0,
                            child: Video::asset("docs/video1.mp4"),
                        ),
                    ],
                ),
            ],
        ),
    )
    .padding_all(tokens.spacing.s)
    .bg(tokens.colors.surface)
    .border(tokens.colors.border, 1.0)
    .border_radius(tokens.radii.medium)
}

fission_component! {
/// A search tip with an example query and keyboard shortcut.
    pub(super) PowerTip<()> |_tip, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
    Card(
        child: Container(
            VStack(
                spacing: tokens.spacing.s,
                children: [
                    Text(TextContent::Key("email.power_tip".into()))
                        .size(tokens.typography.font_size_base),
                    Code(text: "label:important after:2025/01/01"),
                    HStack(
                        spacing: tokens.spacing.xs,
                        children: [
                            Kbd(text: "g"),
                            Kbd(text: "i"),
                            Text(cx.tr("email.jump_to_inbox"))
                                .size(tokens.typography.font_size_xs),
                        ],
                    ),
                ],
            ),
        )
        .padding_all(tokens.spacing.s),
    )
}
