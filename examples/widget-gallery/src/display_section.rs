use fission::core::ui::{
    BadgeTone, ContextMenu, ContextMenuEntry, ContextMenuItem, ContextMenuRegion, Image,
};
use fission::op::{BoxAlignment, ImageFit};
use fission::prelude::*;
use fission::widgets::divider::Orientation;
use fission::widgets::{
    AspectRatio, Avatar, AvatarGroup, AvatarGroupItem, Badge, Code, Divider, HStack, Kbd,
    MarkdownContent, Stat, Tag, VStack, Wrap,
};

const AVATAR_SIZE: f32 = 36.0;
const AVATAR_GROUP_VISIBLE: usize = 3;
const BORDER_WIDTH: f32 = 1.0;
const DIVIDER_DASH: f32 = 4.0;
const WIDE_RATIO_WIDTH: f32 = 320.0;
const SQUARE_RATIO_WIDTH: f32 = 160.0;
const IMAGE_WIDTH: f32 = 240.0;
const IMAGE_HEIGHT: f32 = 160.0;
const TEAM: &[(&str, &str)] = &[
    ("ada", "Ada Lovelace"),
    ("grace", "Grace Hopper"),
    ("alan", "Alan Turing"),
    ("katherine", "Katherine Johnson"),
    ("linus", "Linus Torvalds"),
];
const BADGE_TONES: &[(BadgeTone, &str)] = &[
    (BadgeTone::Brand, "New"),
    (BadgeTone::Gray, "Draft"),
    (BadgeTone::Success, "Paid"),
    (BadgeTone::Warning, "Pending"),
    (BadgeTone::Error, "Overdue"),
    (BadgeTone::Blue, "Info"),
    (BadgeTone::Orange, "Beta"),
];
const MARKDOWN_SAMPLE: &str = "# Release notes\n\n\
Version **2.4** makes sharing faster and fixes a few rough edges.\n\n\
## What's new\n\n\
- Share a page with a link\n\
- Pick a *date range* when filtering\n\
- Rename documents in place\n\n\
> Tip: press `Ctrl+K` to search from anywhere.\n\n\
Read the [full changelog](https://example.com/changelog) for details.\n\n\
```rust\nlet total = items.iter().sum::<u32>();\n```\n";

pub(crate) fn avatar() -> Vec<Widget> {
    widgets![AvatarDemo::new()]
}

fission_component! {
    AvatarDemo<()> |_demo, _state, cx| { let tokens = &cx.view.env().theme.tokens; } =>
    Column(children: [
        HStack(
            spacing: tokens.spacing.s,
            children: [
                Avatar(name: "John Doe".to_string(), size: AVATAR_SIZE),
                Avatar(name: "Ada Lovelace".to_string(), size: AVATAR_SIZE),
            ],
        ),
        // A group overlaps its people and counts the ones it cannot show.
        HStack(
            spacing: None,
            children: [AvatarGroup(
                id: WidgetId::explicit("gallery.avatar.group"),
                avatars: TEAM
                    .iter()
                    .map(|(id, name)| AvatarGroupItem::new(
                        WidgetId::explicit(&format!("gallery.avatar.{id}")),
                        Avatar {
                            name: Some((*name).into()),
                            ..Default::default()
                        },
                    ))
                    .collect::<Vec<_>>(),
                max_visible: AVATAR_GROUP_VISIBLE,
                size: AVATAR_SIZE,
                semantics_label: "Project members".to_string(),
            )],
        ),
    ])
}

pub(crate) fn badge() -> Vec<Widget> {
    widgets![BadgeDemo::new()]
}

fission_component! {
    BadgeDemo<()> |_demo, _state, cx| { let tokens = &cx.view.env().theme.tokens; } =>
    Wrap(
        direction: FlexDirection::Row,
        spacing: tokens.spacing.s,
        run_spacing: tokens.spacing.s,
        children: [
            for (tone, label) in BADGE_TONES.iter() {
                Badge(text: (*label).to_string(), tone: *tone, key: *label)
            },
        ],
    )
}

pub(crate) fn divider() -> Vec<Widget> {
    widgets![DividerDemo::new()]
}

fission_component! {
    DividerDemo<()> |_demo, _state, cx| { let tokens = &cx.view.env().theme.tokens; } =>
    Column(children: [
        VStack(
            spacing: tokens.spacing.s,
            children: [
                Text("Account").color(tokens.colors.text_primary),
                Divider(),
                Text("Notifications").color(tokens.colors.text_primary),
            ],
        ),
        HStack(
            spacing: None,
            children: [HStack(
                spacing: tokens.spacing.m,
                children: [
                    Text("Edit").color(tokens.colors.text_primary),
                    Container(
                        Divider(orientation: Orientation::Vertical),
                        key: "edit-share-divider",
                    )
                    .height(tokens.sizing.icon_md),
                    Text("Share").color(tokens.colors.text_primary),
                    Container(
                        Divider(orientation: Orientation::Vertical),
                        key: "share-delete-divider",
                    )
                    .height(tokens.sizing.icon_md),
                    Text("Delete").color(tokens.colors.text_primary),
                ],
            )],
        ),
        Divider(dash_pattern: vec![DIVIDER_DASH, DIVIDER_DASH]),
    ])
}

fission_component! {
    RatioTile<()> { width: f32, ratio: f32, label: &'static str }
    |tile, _state, cx| { let tokens = &cx.view.env().theme.tokens; } =>
    Container(AspectRatio(
        ratio: tile.ratio,
        child: Container(Text(tile.label).color(tokens.colors.text_primary))
            .width_length(Length::percent(100.0))
            .height_length(Length::percent(100.0))
            .align_child(BoxAlignment::Center)
            .bg(tokens.colors.primary_subtle)
            .border_radius(tokens.radii.medium),
    ))
    .width(tile.width)
}

pub(crate) fn aspect_ratio() -> Vec<Widget> {
    widgets![AspectRatioDemo::new()]
}

fission_component! {
    AspectRatioDemo<()> |_demo, _state, cx| { let tokens = &cx.view.env().theme.tokens; } =>
    Row(
        gap: tokens.spacing.m,
        align_items: fission::op::AlignItems::Start,
        children: [
            RatioTile(WIDE_RATIO_WIDTH, 16.0_f32 / 9.0_f32, "16 : 9"),
            RatioTile(SQUARE_RATIO_WIDTH, 1.0_f32, "1 : 1"),
        ],
    )
}

fn hex(color: Color) -> String {
    format!("#{:02x}{:02x}{:02x}", color.r, color.g, color.b)
}

pub(crate) fn image() -> Vec<Widget> {
    widgets![ImageDemo::new()]
}

fission_component! {
    ImageDemo<()> |_demo, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let colors = &tokens.colors;
        let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 240 160">
<rect width="240" height="160" fill="{sky}"/>
<circle cx="182" cy="46" r="20" fill="{sun}"/>
<path d="M0 160 L78 62 L130 124 L166 88 L240 160 Z" fill="{hill}"/>
</svg>"#,
        sky = hex(colors.primary_subtle),
        sun = hex(colors.warning),
        hill = hex(colors.primary),
        );
    } =>
    Wrap(
        direction: FlexDirection::Row,
        spacing: tokens.spacing.m,
        run_spacing: tokens.spacing.m,
        children: [
            ImageFrame(svg.clone(), ImageFit::Cover, "Cover"),
            ImageFrame(svg, ImageFit::Contain, "Contain"),
        ],
    )
}

fission_component! {
    ImageFrame<()> { svg: String, fit: ImageFit, label: &'static str }
    |frame, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let colors = &tokens.colors;
    } =>
    VStack(
        spacing: tokens.spacing.xs,
        children: [
            Container(
                Image::svg_text(frame.svg.clone())
                    .size(IMAGE_WIDTH, IMAGE_HEIGHT)
                    .fit(frame.fit)
                    .semantic_label("Mountains under the sun"),
            )
            .size(IMAGE_WIDTH, IMAGE_HEIGHT)
            .bg(colors.surface)
            .border(colors.border, tokens.sizing.border_hairline)
            .border_radius(tokens.radii.medium),
            Text(frame.label).color(colors.text_secondary),
        ],
    )
}

pub(crate) fn markdown() -> Vec<Widget> {
    widgets![MarkdownContent::new(MARKDOWN_SAMPLE)]
}

pub(crate) fn tag() -> Vec<Widget> {
    widgets![TagDemo::new()]
}

fission_component! {
    TagDemo<()> |_demo, _state, cx| { let tokens = &cx.view.env().theme.tokens; } =>
    HStack(
        spacing: tokens.spacing.s,
        children: [
            Tag(
                label: "Rust",
                selected: false,
            ),
            Tag(
                label: "Design",
                selected: true,
            ),
        ],
    )
}

pub(crate) fn code() -> Vec<Widget> {
    widgets![CodeDemo::new()]
}

fission_component! {
    CodeDemo<()> |_demo, _state, cx| { let tokens = &cx.view.env().theme.tokens; } =>
    HStack(
        spacing: tokens.spacing.s,
        children: [
            Code(
                text: "let x = 42;",
            ),
            Kbd(
                text: "Ctrl+C",
            ),
        ],
    )
}

pub(crate) fn stat() -> Vec<Widget> {
    widgets![StatDemo::new()]
}

fission_component! {
    StatDemo<()> |_demo, _state, _cx| {} =>
    HStack(
        spacing: None,
        children: [Stat(
            label: "Total Users",
            value: "1,234",
            help_text: Some("+12% this month".to_string()),
        )],
    )
}

fission_component! {
    ContextMenuTarget<()> |_target, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Container(
            Text("Right-click this custom region for a widget-backed context menu.")
                .wrap(true)
        )
        .padding_lengths(Length::all(Length::points(tokens.spacing.s)))
        .border(tokens.colors.border, BORDER_WIDTH)
        .border_radius(tokens.radii.large)
}

pub(crate) fn context_menu() -> Vec<Widget> {
    widgets![ContextMenuDemo::new()]
}

fission_component! {
    ContextMenuDemo<()> |_demo, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let typography = &tokens.typography;
    } =>
    Column(children: [
        Container(Text(
            id: WidgetId::explicit("gallery.selectable.text"),
            content: TextContent::Literal(
                "Selectable Text: drag across this sentence, then use Ctrl/Cmd+C or right-click for Copy and Select All.".into(),
            ),
            selectable: true,
            wrap: true,
        ))
        .width_length(Length::percent(100.0)),
        Container(RichText(
            id: WidgetId::explicit("gallery.selectable.rich_text"),
            runs: vec![
                RichTextRun::new("Selectable RichText: "),
                RichTextRun::new("mixed style text can be selected too.")
                    .color(tokens.colors.text_link)
                    .weight(typography.font_weight_bold),
            ],
            selectable: true,
            wrap: true,
        ))
        .width_length(Length::percent(100.0)),
        ContextMenuRegion::new(
            ContextMenuTarget::new(),
            ContextMenu::with_items([ContextMenuEntry::Item(ContextMenuItem::new(
                "custom-help",
                ContextMenuHelp::new(),
            ))]),
        )
        .id(WidgetId::explicit("gallery.custom.context_menu")),
    ])
}

fission_component! {
    ContextMenuHelp<()> |_help, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
    HStack(
        spacing: tokens.spacing.xs,
        children: [
            Badge(text: "Tip".to_string()),
            Text(TextContent::KeyWithFallback {
                key: "gallery.context_menu.copy_help".into(),
                fallback: "Menu items can be arbitrary widgets".into(),
            }),
        ],
    )
}
