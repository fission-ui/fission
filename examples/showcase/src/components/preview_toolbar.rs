use crate::i18n::message;
use crate::semantics::ShowcaseSemantics;
use crate::state::{
    on_reset_preview, on_set_locale, on_set_preview_viewport, on_set_theme, PreviewViewport,
    ResetPreview, SetLocale, SetPreviewViewport, SetTheme, ShowcaseState,
};
use fission::icons::material;
use fission::op::{AlignItems, Fill, FlexWrap};
use fission::prelude::*;

#[derive(Clone, Debug)]
pub(crate) struct PreviewToolbar;

impl From<PreviewToolbar> for Widget {
    fn from(_component: PreviewToolbar) -> Self {
        let (ctx, view) = fission::build::current::<ShowcaseState>();
        let tokens = &view.env().theme.tokens;
        let reset = with_reducer!(ctx, ResetPreview, on_reset_preview);
        let desktop = with_reducer!(
            ctx,
            SetPreviewViewport(PreviewViewport::Desktop),
            on_set_preview_viewport
        );
        let mobile = with_reducer!(
            ctx,
            SetPreviewViewport(PreviewViewport::Mobile),
            on_set_preview_viewport
        );
        let english = with_reducer!(ctx, SetLocale("en-US".into()), on_set_locale);
        let spanish = with_reducer!(ctx, SetLocale("es-ES".into()), on_set_locale);
        let light = with_reducer!(ctx, SetTheme(DesignMode::Light), on_set_theme);
        let dark = with_reducer!(ctx, SetTheme(DesignMode::Dark), on_set_theme);
        Container::new(Row {
            children: widgets![
                Button {
                    variant: ButtonVariant::TertiaryGray,
                    size: ComponentSize::Sm,
                    child: Some(
                        Row {
                            children: widgets![
                                Icon::svg(material::device::restart_alt::round())
                                    .size(tokens.typography.font_size_lg),
                                Text::new(TextContent::Key("showcase.workbench.reset".into())),
                            ],
                            gap: Some(tokens.spacing.xs),
                            align_items: AlignItems::Center,
                            ..Default::default()
                        }
                        .into(),
                    ),
                    on_press: Some(reset),
                    semantics: Some(
                        Semantics::button(message(view.env(), "showcase.workbench.reset"))
                            .identifier("showcase.preview.reset"),
                    ),
                    ..Default::default()
                },
                Button {
                    variant: if view.state().preview_viewport == PreviewViewport::Desktop {
                        ButtonVariant::SecondaryColor
                    } else {
                        ButtonVariant::TertiaryGray
                    },
                    size: ComponentSize::Sm,
                    child: Some(
                        Icon::svg(material::hardware::computer::round())
                            .size(tokens.typography.font_size_lg)
                            .into(),
                    ),
                    on_press: Some(desktop),
                    semantics: Some(
                        Semantics::button(message(
                            view.env(),
                            "showcase.workbench.viewport.desktop",
                        ))
                        .identifier("showcase.preview.viewport.desktop"),
                    ),
                    ..Default::default()
                },
                Button {
                    variant: if view.state().preview_viewport == PreviewViewport::Mobile {
                        ButtonVariant::SecondaryColor
                    } else {
                        ButtonVariant::TertiaryGray
                    },
                    size: ComponentSize::Sm,
                    child: Some(
                        Icon::svg(material::hardware::smartphone::round())
                            .size(tokens.typography.font_size_lg)
                            .into(),
                    ),
                    on_press: Some(mobile),
                    semantics: Some(
                        Semantics::button(message(
                            view.env(),
                            "showcase.workbench.viewport.mobile",
                        ))
                        .identifier("showcase.preview.viewport.mobile"),
                    ),
                    ..Default::default()
                },
                Button {
                    variant: if view.state().locale.0 == "en-US" {
                        ButtonVariant::SecondaryColor
                    } else {
                        ButtonVariant::TertiaryGray
                    },
                    size: ComponentSize::Sm,
                    child: Some(Text::new("EN").into()),
                    on_press: Some(english),
                    semantics: Some(
                        Semantics::button("English").identifier("showcase.preview.locale.english"),
                    ),
                    ..Default::default()
                },
                Button {
                    variant: if view.state().locale.0 == "es-ES" {
                        ButtonVariant::SecondaryColor
                    } else {
                        ButtonVariant::TertiaryGray
                    },
                    size: ComponentSize::Sm,
                    child: Some(Text::new("ES").into()),
                    on_press: Some(spanish),
                    semantics: Some(
                        Semantics::button("Español").identifier("showcase.preview.locale.spanish"),
                    ),
                    ..Default::default()
                },
                Button {
                    variant: if view.state().theme_mode == DesignMode::Light {
                        ButtonVariant::SecondaryColor
                    } else {
                        ButtonVariant::TertiaryGray
                    },
                    size: ComponentSize::Sm,
                    child: Some(
                        Icon::svg(material::image::wb_sunny::round())
                            .size(tokens.typography.font_size_lg)
                            .into(),
                    ),
                    on_press: Some(light),
                    semantics: Some(
                        Semantics::button(message(view.env(), "showcase.workbench.light"))
                            .identifier("showcase.preview.theme.light"),
                    ),
                    ..Default::default()
                },
                Button {
                    variant: if view.state().theme_mode == DesignMode::Dark {
                        ButtonVariant::SecondaryColor
                    } else {
                        ButtonVariant::TertiaryGray
                    },
                    size: ComponentSize::Sm,
                    child: Some(
                        Icon::svg(material::image::brightness_3::round())
                            .size(tokens.typography.font_size_lg)
                            .into(),
                    ),
                    on_press: Some(dark),
                    semantics: Some(
                        Semantics::button(message(view.env(), "showcase.workbench.dark"))
                            .identifier("showcase.preview.theme.dark"),
                    ),
                    ..Default::default()
                },
            ],
            gap: Some(tokens.spacing.xs),
            wrap: FlexWrap::Wrap,
            align_items: AlignItems::Center,
            ..Default::default()
        })
        .padding_lengths(Length::all(Length::points(tokens.spacing.s)))
        .bg_fill(Fill::Solid(tokens.colors.surface_sunken))
        .border_radius(tokens.radii.large)
        .into()
    }
}
