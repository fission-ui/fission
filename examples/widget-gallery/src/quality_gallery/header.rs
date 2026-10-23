use fission::prelude::*;

use crate::GalleryState;

#[fission_reducer(ToggleQualityMenu)]
fn toggle_quality_menu(state: &mut GalleryState) {
    state.menu_open = !state.menu_open;
    state.select_open = false;
    state.modal_open = false;
}

#[fission_reducer(CloseQualityMenu)]
fn close_quality_menu(state: &mut GalleryState) {
    state.menu_open = false;
}

fission_component! {
    pub(super) QualityHeader<GalleryState> { compact: bool }
    |header, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let close_menu = with_reducer!(cx.build, CloseQualityMenu, close_quality_menu);
        let toggle_menu = with_reducer!(cx.build, ToggleQualityMenu, toggle_quality_menu);
        let menu_id = WidgetId::explicit(if header.compact {
            "quality-gallery.menu.mobile"
        } else {
            "quality-gallery.menu.desktop"
        });

    } =>
        Column(
            gap: 0.0_f32,
            children: [
                Container(Row(
                    gap: tokens.spacing.s,
                    align_items: fission::op::AlignItems::Center,
                    children: [
                        Container(
                            Icon::svg(material::action::settings::regular())
                                .size(14.0)
                                .color(tokens.colors.on_primary),
                        )
                        .size(24.0, 24.0)
                        .align_child(BoxAlignment::Center)
                        .bg(tokens.colors.primary)
                        .border_radius(tokens.radii.medium),
                        Text("Northstar")
                            .size(tokens.typography.font_size_base)
                            .line_height(20.0)
                            .weight(tokens.typography.font_weight_medium)
                            .color(tokens.colors.text_primary),
                        Spacer(flex_grow: 1.0_f32),
                        MenuButtonLayout::new(
                            menu_id,
                            MenuTrigger::new("Actions")
                                .size(ComponentSize::Sm)
                                .semantics_identifier("quality-gallery.menu.trigger"),
                            MenuContent::new(vec![
                                MenuGroup::new(vec![
                                    MenuActionItem::new("Edit profile")
                                        .leading_icon(material::social::person::regular())
                                        .shortcut("⌘E")
                                        .on_select(close_menu.clone())
                                        .semantics_identifier("quality-gallery.menu.edit-profile",)
                                        .into(),
                                    MenuActionItem::new("Change password")
                                        .leading_icon(material::communication::vpn_key::regular(),)
                                        .on_select(close_menu.clone())
                                        .semantics_identifier(
                                            "quality-gallery.menu.change-password",
                                        )
                                        .into(),
                                ])
                                .label("Account actions")
                                .into(),
                                MenuSeparator::new().into(),
                                MenuActionItem::new("Delete account")
                                    .leading_icon(material::action::delete::regular())
                                    .on_select(close_menu)
                                    .semantics_identifier("quality-gallery.menu.delete-account")
                                    .tone(MenuItemTone::Destructive)
                                    .into(),
                            ])
                            .width(208.0),
                        )
                        .open(state.menu_open)
                        .on_toggle(toggle_menu),
                    ],
                ))
                .height(35.0),
                Divider::default(),
            ],
        )
}
