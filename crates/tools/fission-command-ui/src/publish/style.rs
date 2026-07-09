use super::*;

#[derive(Clone, Copy)]
pub(crate) struct PublishPalette {
    pub(crate) background: Color,
    pub(crate) background_alt: Color,
    pub(crate) panel: Color,
    pub(crate) panel_soft: Color,
    pub(crate) input: Color,
    pub(crate) hairline: Color,
    pub(crate) text: Color,
    pub(crate) muted: Color,
    pub(crate) subtle: Color,
    pub(crate) accent: Color,
    pub(crate) accent_text: Color,
    pub(crate) blue: Color,
    pub(crate) success: Color,
    pub(crate) warning: Color,
    pub(crate) error: Color,
}

impl PublishPalette {
    pub(crate) fn for_mode(mode: ThemeMode) -> Self {
        match mode {
            ThemeMode::Dark => Self {
                background: rgb(5, 10, 15),
                background_alt: rgb(9, 17, 24),
                panel: rgb(15, 24, 34),
                panel_soft: rgb(22, 36, 48),
                input: rgb(9, 16, 24),
                hairline: rgb(52, 68, 84),
                text: rgb(242, 247, 246),
                muted: rgb(163, 178, 187),
                subtle: rgb(104, 122, 133),
                accent: rgb(93, 213, 112),
                accent_text: rgb(5, 22, 12),
                blue: rgb(77, 142, 246),
                success: rgb(93, 213, 112),
                warning: rgb(244, 193, 70),
                error: rgb(246, 84, 80),
            },
            ThemeMode::Light => Self {
                background: rgb(238, 243, 240),
                background_alt: rgb(229, 237, 233),
                panel: rgb(250, 252, 249),
                panel_soft: rgb(237, 245, 239),
                input: rgb(255, 255, 252),
                hairline: rgb(178, 194, 184),
                text: rgb(17, 29, 33),
                muted: rgb(80, 95, 101),
                subtle: rgb(116, 134, 140),
                accent: rgb(24, 148, 72),
                accent_text: rgb(245, 255, 248),
                blue: rgb(35, 101, 210),
                success: rgb(24, 148, 72),
                warning: rgb(184, 117, 22),
                error: rgb(200, 54, 55),
            },
        }
    }
}

pub(crate) fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color { r, g, b, a: 255 }
}

pub(crate) fn theme_for_mode(mode: ThemeMode) -> fission::theme::Theme {
    match mode {
        ThemeMode::Dark => fission::theme::Theme::dark(),
        ThemeMode::Light => fission::theme::Theme::default(),
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct PublishLayout {
    pub(crate) terminal: bool,
    pub(crate) compact: bool,
    pub(crate) root_padding: f32,
    pub(crate) gap: f32,
    pub(crate) header_height: f32,
    pub(crate) footer_height: f32,
    pub(crate) body_height: f32,
    pub(crate) panel_radius: f32,
    pub(crate) card_padding: f32,
    pub(crate) column_width: f32,
    pub(crate) content_max_width: f32,
    pub(crate) icon_size: f32,
    pub(crate) button_height: f32,
    pub(crate) input_height: f32,
    pub(crate) control_radius: f32,
}

impl PublishLayout {
    pub(crate) fn from_viewport(size: fission::LayoutSize) -> Self {
        let terminal = size.width < 260.0;
        let compact = terminal || size.width < 980.0;
        let unit = if terminal {
            1.0
        } else {
            (size.width / 190.0).clamp(6.0, 9.0)
        };
        let root_padding = if terminal { 1.0 } else { 2.1 * unit };
        let gap = if terminal { 1.0 } else { 1.25 * unit };
        let header_height = if terminal {
            5.0
        } else if compact {
            118.0
        } else {
            104.0
        };
        let footer_height = if terminal { 3.0 } else { 48.0 };
        let body_height =
            (size.height - (root_padding * 2.0) - header_height - footer_height - (gap * 2.0))
                .max(if terminal { 28.0 } else { 420.0 });
        let column_width = if terminal {
            (size.width - root_padding * 2.0).max(60.0)
        } else {
            ((size.width - root_padding * 2.0 - gap * 3.0) / 4.0).max(260.0)
        };
        Self {
            terminal,
            compact,
            root_padding,
            gap,
            header_height,
            footer_height,
            body_height,
            panel_radius: if terminal { 1.0 } else { 12.0 },
            card_padding: if terminal { 1.0 } else { 14.0 },
            column_width,
            content_max_width: if terminal { column_width } else { 1180.0 },
            icon_size: if terminal { 10.0 } else { 18.0 },
            button_height: if terminal { 3.0 } else { 38.0 },
            input_height: if terminal { 3.0 } else { 54.0 },
            control_radius: if terminal { 1.0 } else { 7.0 },
        }
    }

    pub(crate) fn wizard_width(self, viewport_width: f32) -> f32 {
        if self.terminal {
            self.column_width
        } else {
            (viewport_width - self.root_padding * 2.0).min(self.content_max_width)
        }
    }
}
