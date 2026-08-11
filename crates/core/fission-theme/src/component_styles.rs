use super::*;

// --- Component Themes ---

/// Visual parameters for the `Button` widget.
///
/// Includes dimensions, padding, corner radius, text size, elevation for
/// rest/hover/pressed states, and an optional focus stroke.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ButtonTheme {
    pub height: f32,
    pub padding_horizontal: f32,
    pub padding_vertical: f32,
    pub radius: f32,
    pub text_size: f32,
    pub elevation_rest: Option<BoxShadow>,
    pub elevation_hover: Option<BoxShadow>,
    pub elevation_pressed: Option<BoxShadow>,
    pub focus_stroke: Option<Stroke>,
    pub icon_size: f32,
    pub font_weight: u16,
    pub line_height: f32,
    pub transition: Option<ComponentMotion>,
    pub sizes: Vec<(ComponentSize, ResolvedComponentStyle)>,
    pub hierarchies: Vec<(ButtonHierarchy, ComponentStateStyles)>,
}

impl ButtonTheme {
    pub fn from_tokens(tokens: &Tokens) -> Self {
        let transition = Some(ComponentMotion {
            duration_ms: tokens.motion.duration_fast_ms,
            easing: tokens.motion.easing_standard.clone(),
        });
        let size_md = ResolvedComponentStyle {
            height: Some(40.0),
            padding_x: Some(14.0),
            padding_y: Some(tokens.spacing.s),
            gap: Some(4.0),
            font_size: Some(tokens.typography.label_large_size),
            font_weight: Some(tokens.typography.font_weight_semibold),
            line_height: Some(20.0),
            icon_size: Some(20.0),
            ..ResolvedComponentStyle::default()
        };
        let primary = ComponentStateStyles {
            default: ResolvedComponentStyle {
                background: Some(Fill::Solid(tokens.colors.primary)),
                text_color: Some(tokens.colors.on_primary),
                border: None,
                shadows: tokens
                    .elevations
                    .level1
                    .map(shadow_layer_from_box)
                    .into_iter()
                    .collect(),
                transition: transition.clone(),
                ..ResolvedComponentStyle::default()
            },
            hover: Some(ResolvedComponentStyle {
                background: Some(Fill::Solid(tokens.colors.primary_hover)),
                shadows: tokens
                    .elevations
                    .level2
                    .map(shadow_layer_from_box)
                    .into_iter()
                    .collect(),
                ..ResolvedComponentStyle::default()
            }),
            active: Some(ResolvedComponentStyle {
                shadows: tokens
                    .elevations
                    .level0
                    .map(shadow_layer_from_box)
                    .into_iter()
                    .collect(),
                ..ResolvedComponentStyle::default()
            }),
            focus: Some(ResolvedComponentStyle {
                shadows: tokens
                    .elevations
                    .focus
                    .map(shadow_layer_from_box)
                    .into_iter()
                    .collect(),
                ..ResolvedComponentStyle::default()
            }),
            disabled: Some(ResolvedComponentStyle {
                background: Some(Fill::Solid(tokens.colors.border)),
                text_color: Some(tokens.colors.text_secondary),
                shadows: Vec::new(),
                ..ResolvedComponentStyle::default()
            }),
            ..ComponentStateStyles::default()
        };
        let secondary_gray = ComponentStateStyles {
            default: ResolvedComponentStyle {
                background: Some(Fill::Solid(tokens.colors.surface)),
                text_color: Some(tokens.colors.text_primary),
                border: Some(ComponentBorder {
                    fill: Fill::Solid(tokens.colors.border),
                    width: 1.0,
                }),
                transition: transition.clone(),
                ..ResolvedComponentStyle::default()
            },
            hover: Some(ResolvedComponentStyle {
                background: Some(Fill::Solid(tokens.colors.surface_sunken)),
                ..ResolvedComponentStyle::default()
            }),
            disabled: Some(ResolvedComponentStyle {
                text_color: Some(tokens.colors.text_secondary),
                border: Some(ComponentBorder {
                    fill: Fill::Solid(tokens.colors.border),
                    width: 1.0,
                }),
                ..ResolvedComponentStyle::default()
            }),
            ..ComponentStateStyles::default()
        };
        let tertiary_gray = ComponentStateStyles {
            default: ResolvedComponentStyle {
                background: None,
                text_color: Some(tokens.colors.primary),
                border: None,
                transition,
                ..ResolvedComponentStyle::default()
            },
            hover: Some(ResolvedComponentStyle {
                background: Some(Fill::Solid(tokens.colors.surface_sunken)),
                ..ResolvedComponentStyle::default()
            }),
            disabled: Some(ResolvedComponentStyle {
                text_color: Some(tokens.colors.text_secondary),
                ..ResolvedComponentStyle::default()
            }),
            ..ComponentStateStyles::default()
        };
        Self {
            height: 42.0,
            padding_horizontal: tokens.spacing.m,
            padding_vertical: tokens.spacing.s,
            radius: tokens.radii.full,
            text_size: tokens.typography.label_large_size,
            elevation_rest: tokens.elevations.level1,
            elevation_hover: tokens.elevations.level2,
            elevation_pressed: tokens.elevations.level0,
            focus_stroke: Some(Stroke {
                fill: fission_ir::op::Fill::Solid(tokens.colors.on_background),
                width: 1.0,
                dash_array: None,
                line_cap: fission_ir::op::LineCap::Butt,
                line_join: fission_ir::op::LineJoin::Miter,
            }),
            icon_size: 20.0,
            font_weight: tokens.typography.font_weight_semibold,
            line_height: 20.0,
            transition: Some(ComponentMotion {
                duration_ms: tokens.motion.duration_fast_ms,
                easing: tokens.motion.easing_standard.clone(),
            }),
            sizes: vec![
                (
                    ComponentSize::Sm,
                    ResolvedComponentStyle {
                        height: Some(36.0),
                        padding_x: Some(12.0),
                        padding_y: Some(tokens.spacing.xs),
                        gap: Some(4.0),
                        font_size: Some(tokens.typography.font_size_sm),
                        line_height: Some(20.0),
                        icon_size: Some(18.0),
                        ..ResolvedComponentStyle::default()
                    },
                ),
                (ComponentSize::Md, size_md),
                (
                    ComponentSize::Lg,
                    ResolvedComponentStyle {
                        height: Some(44.0),
                        padding_x: Some(16.0),
                        padding_y: Some(tokens.spacing.s),
                        gap: Some(6.0),
                        font_size: Some(tokens.typography.font_size_base),
                        line_height: Some(24.0),
                        icon_size: Some(20.0),
                        ..ResolvedComponentStyle::default()
                    },
                ),
                (
                    ComponentSize::Xl,
                    ResolvedComponentStyle {
                        height: Some(48.0),
                        padding_x: Some(18.0),
                        padding_y: Some(tokens.spacing.s),
                        gap: Some(6.0),
                        font_size: Some(tokens.typography.font_size_base),
                        line_height: Some(24.0),
                        icon_size: Some(20.0),
                        ..ResolvedComponentStyle::default()
                    },
                ),
            ],
            hierarchies: vec![
                (ButtonHierarchy::Primary, primary.clone()),
                (ButtonHierarchy::SecondaryColor, secondary_gray.clone()),
                (ButtonHierarchy::SecondaryGray, secondary_gray),
                (ButtonHierarchy::TertiaryColor, tertiary_gray.clone()),
                (ButtonHierarchy::TertiaryGray, tertiary_gray.clone()),
                (ButtonHierarchy::LinkColor, tertiary_gray.clone()),
                (ButtonHierarchy::LinkGray, tertiary_gray.clone()),
                (
                    ButtonHierarchy::Destructive,
                    ComponentStateStyles {
                        default: ResolvedComponentStyle {
                            background: Some(Fill::Solid(tokens.colors.error)),
                            text_color: Some(tokens.colors.on_error),
                            ..primary.default.clone()
                        },
                        hover: Some(ResolvedComponentStyle {
                            background: Some(Fill::Solid(tokens.colors.error.with_alpha(230))),
                            ..ResolvedComponentStyle::default()
                        }),
                        ..primary
                    },
                ),
            ],
        }
    }

    pub fn size_style(&self, size: ComponentSize) -> ResolvedComponentStyle {
        self.sizes
            .iter()
            .find(|(candidate, _)| *candidate == size)
            .map(|(_, style)| style.clone())
            .or_else(|| {
                self.sizes
                    .iter()
                    .find(|(candidate, _)| *candidate == ComponentSize::Md)
                    .map(|(_, style)| style.clone())
            })
            .unwrap_or_else(|| ResolvedComponentStyle {
                height: Some(self.height),
                padding_x: Some(self.padding_horizontal),
                padding_y: Some(self.padding_vertical),
                radius: Some(self.radius),
                font_size: Some(self.text_size),
                font_weight: Some(self.font_weight),
                line_height: Some(self.line_height),
                icon_size: Some(self.icon_size),
                ..ResolvedComponentStyle::default()
            })
    }

    pub fn hierarchy_style(&self, hierarchy: ButtonHierarchy) -> ComponentStateStyles {
        self.hierarchies
            .iter()
            .find(|(candidate, _)| *candidate == hierarchy)
            .map(|(_, styles)| styles.clone())
            .or_else(|| {
                self.hierarchies
                    .iter()
                    .find(|(candidate, _)| *candidate == ButtonHierarchy::Primary)
                    .map(|(_, styles)| styles.clone())
            })
            .unwrap_or_default()
    }

    pub fn resolve(
        &self,
        hierarchy: ButtonHierarchy,
        size: ComponentSize,
        state: ComponentState,
    ) -> ResolvedComponentStyle {
        let base = ResolvedComponentStyle {
            height: Some(self.height),
            padding_x: Some(self.padding_horizontal),
            padding_y: Some(self.padding_vertical),
            radius: Some(self.radius),
            font_size: Some(self.text_size),
            font_weight: Some(self.font_weight),
            line_height: Some(self.line_height),
            icon_size: Some(self.icon_size),
            transition: self.transition.clone(),
            ..ResolvedComponentStyle::default()
        };
        base.merge(&self.size_style(size))
            .merge(&self.hierarchy_style(hierarchy).resolve(state))
    }
}

/// Visual parameters for the `TextInput` widget.
///
/// Controls height, horizontal padding, corner radius, font size, and colors
/// for border, focus ring, text, and placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TextInputTheme {
    pub height: f32,
    pub padding_h: f32,
    pub radius: f32,
    pub font_size: f32,
    pub border_color: Color,
    pub border_width: f32,
    pub focus_color: Color,
    pub text_color: Color,
    pub placeholder_color: Color,
    pub line_height: f32,
    pub font_weight: u16,
    pub sizes: Vec<(ComponentSize, ResolvedComponentStyle)>,
    pub states: ComponentStateStyles,
    pub placeholder_style: ResolvedComponentStyle,
    pub label_style: ResolvedComponentStyle,
    pub helper_style: ResolvedComponentStyle,
}

impl TextInputTheme {
    pub fn from_tokens(tokens: &Tokens) -> Self {
        Self {
            height: 40.0,
            padding_h: tokens.spacing.m,
            radius: tokens.radii.small,
            font_size: tokens.typography.body_large_size,
            border_color: tokens.colors.border,
            border_width: 1.0,
            focus_color: tokens.colors.primary,
            text_color: tokens.colors.text_primary,
            placeholder_color: tokens.colors.text_secondary,
            line_height: 24.0,
            font_weight: tokens.typography.font_weight_regular,
            sizes: vec![
                (
                    ComponentSize::Sm,
                    ResolvedComponentStyle {
                        height: Some(36.0),
                        padding_x: Some(12.0),
                        ..ResolvedComponentStyle::default()
                    },
                ),
                (
                    ComponentSize::Md,
                    ResolvedComponentStyle {
                        height: Some(40.0),
                        padding_x: Some(12.0),
                        ..ResolvedComponentStyle::default()
                    },
                ),
            ],
            states: ComponentStateStyles {
                default: ResolvedComponentStyle {
                    background: Some(Fill::Solid(tokens.colors.surface)),
                    text_color: Some(tokens.colors.text_primary),
                    border: Some(ComponentBorder {
                        fill: Fill::Solid(tokens.colors.border),
                        width: 1.0,
                    }),
                    shadows: tokens
                        .elevations
                        .level1
                        .map(shadow_layer_from_box)
                        .into_iter()
                        .collect(),
                    ..ResolvedComponentStyle::default()
                },
                focus: Some(ResolvedComponentStyle {
                    border: Some(ComponentBorder {
                        fill: Fill::Solid(tokens.colors.focus_ring),
                        width: 2.0,
                    }),
                    shadows: tokens
                        .elevations
                        .focus
                        .map(shadow_layer_from_box)
                        .into_iter()
                        .collect(),
                    padding_x: Some(11.0),
                    ..ResolvedComponentStyle::default()
                }),
                error: Some(ResolvedComponentStyle {
                    border: Some(ComponentBorder {
                        fill: Fill::Solid(tokens.colors.error),
                        width: 1.0,
                    }),
                    ..ResolvedComponentStyle::default()
                }),
                disabled: Some(ResolvedComponentStyle {
                    background: Some(Fill::Solid(tokens.colors.surface_sunken)),
                    text_color: Some(tokens.colors.text_secondary),
                    ..ResolvedComponentStyle::default()
                }),
                ..ComponentStateStyles::default()
            },
            placeholder_style: ResolvedComponentStyle {
                text_color: Some(tokens.colors.text_muted),
                ..ResolvedComponentStyle::default()
            },
            label_style: ResolvedComponentStyle {
                font_size: Some(tokens.typography.font_size_base),
                font_weight: Some(tokens.typography.font_weight_medium),
                text_color: Some(tokens.colors.text_primary),
                ..ResolvedComponentStyle::default()
            },
            helper_style: ResolvedComponentStyle {
                font_size: Some(tokens.typography.font_size_base),
                text_color: Some(tokens.colors.text_muted),
                ..ResolvedComponentStyle::default()
            },
        }
    }

    pub fn size_style(&self, size: ComponentSize) -> ResolvedComponentStyle {
        self.sizes
            .iter()
            .find(|(candidate, _)| *candidate == size)
            .map(|(_, style)| style.clone())
            .or_else(|| {
                self.sizes
                    .iter()
                    .find(|(candidate, _)| *candidate == ComponentSize::Md)
                    .map(|(_, style)| style.clone())
            })
            .unwrap_or_else(|| ResolvedComponentStyle {
                height: Some(self.height),
                padding_x: Some(self.padding_h),
                ..ResolvedComponentStyle::default()
            })
    }

    pub fn resolve(&self, size: ComponentSize, state: ComponentState) -> ResolvedComponentStyle {
        let base = ResolvedComponentStyle {
            height: Some(self.height),
            padding_x: Some(self.padding_h),
            radius: Some(self.radius),
            font_size: Some(self.font_size),
            line_height: Some(self.line_height),
            font_weight: Some(self.font_weight),
            text_color: Some(self.text_color),
            border: Some(ComponentBorder {
                fill: Fill::Solid(self.border_color),
                width: self.border_width,
            }),
            ..ResolvedComponentStyle::default()
        };
        base.merge(&self.size_style(size))
            .merge(&self.states.resolve(state))
    }
}

/// Visual parameters for the `Calendar` widget.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CalendarTheme {
    pub bg_color: Color,
    pub border_color: Color,
    pub radius: f32,
    pub selected_bg: Color,
    pub selected_text: Color,
    pub today_outline: Color,
}

impl CalendarTheme {
    pub fn from_tokens(tokens: &Tokens) -> Self {
        Self {
            bg_color: tokens.colors.surface,
            border_color: tokens.colors.border,
            radius: tokens.radii.medium,
            selected_bg: tokens.colors.primary,
            selected_text: tokens.colors.on_primary,
            today_outline: tokens.colors.secondary,
        }
    }
}

/// Visual parameters for the `Pagination` widget.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PaginationTheme {
    pub spacing: f32,
    pub active_bg: Color,
    pub active_text: Color,
}

impl PaginationTheme {
    pub fn from_tokens(tokens: &Tokens) -> Self {
        Self {
            spacing: tokens.spacing.s,
            active_bg: tokens.colors.primary,
            active_text: tokens.colors.on_primary,
        }
    }
}

/// Visual parameters for the `Timeline` widget.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TimelineTheme {
    pub dot_size: f32,
    pub line_width: f32,
    pub dot_color: Color,
    pub line_color: Color,
}

impl TimelineTheme {
    pub fn from_tokens(tokens: &Tokens) -> Self {
        Self {
            dot_size: 12.0,
            line_width: 2.0,
            dot_color: tokens.colors.primary,
            line_color: tokens.colors.border,
        }
    }
}

/// Visual parameters for the `SegmentedControl` widget.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SegmentedControlTheme {
    pub bg_color: Color,
    pub border_color: Color,
    pub radius: f32,
    pub active_bg: Color,
    pub active_text: Color,
}

impl SegmentedControlTheme {
    pub fn from_tokens(tokens: &Tokens) -> Self {
        Self {
            bg_color: tokens.colors.surface,
            border_color: tokens.colors.border,
            radius: tokens.radii.full,
            active_bg: tokens.colors.primary,
            active_text: tokens.colors.on_primary,
        }
    }
}

/// Visual parameters for the `Alert` widget, with per-severity background colors.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AlertTheme {
    pub info_bg: Color,
    pub warning_bg: Color,
    pub error_bg: Color,
    pub success_bg: Color,
    pub radius: f32,
}

impl AlertTheme {
    pub fn from_tokens(tokens: &Tokens) -> Self {
        Self {
            info_bg: Color {
                r: 230,
                g: 242,
                b: 255,
                a: 255,
            },
            warning_bg: Color {
                r: 255,
                g: 244,
                b: 229,
                a: 255,
            },
            error_bg: tokens.colors.error.with_alpha(30),
            success_bg: Color {
                r: 237,
                g: 247,
                b: 237,
                a: 255,
            },
            radius: tokens.radii.medium,
        }
    }
}

/// Visual parameters for the `Badge` widget.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BadgeTheme {
    pub radius: f32,
    pub font_size: f32,
    pub font_weight: u16,
    pub sizes: Vec<(ComponentSize, ResolvedComponentStyle)>,
    pub tones: Vec<(BadgeTone, ResolvedComponentStyle)>,
}

impl BadgeTheme {
    pub fn from_tokens(tokens: &Tokens) -> Self {
        Self {
            radius: tokens.radii.full,
            font_size: 10.0,
            font_weight: tokens.typography.font_weight_medium,
            sizes: vec![
                (
                    ComponentSize::Sm,
                    ResolvedComponentStyle {
                        height: Some(20.0),
                        padding_x: Some(8.0),
                        font_size: Some(tokens.typography.font_size_xs),
                        line_height: Some(18.0),
                        ..ResolvedComponentStyle::default()
                    },
                ),
                (
                    ComponentSize::Md,
                    ResolvedComponentStyle {
                        height: Some(24.0),
                        padding_x: Some(10.0),
                        font_size: Some(tokens.typography.font_size_base),
                        line_height: Some(20.0),
                        ..ResolvedComponentStyle::default()
                    },
                ),
            ],
            tones: vec![
                (
                    BadgeTone::Brand,
                    badge_tone(
                        tokens.colors.primary_subtle,
                        tokens.colors.primary,
                        tokens.colors.primary,
                    ),
                ),
                (
                    BadgeTone::Gray,
                    badge_tone(
                        tokens.colors.surface_sunken,
                        tokens.colors.border,
                        tokens.colors.text_primary,
                    ),
                ),
                (
                    BadgeTone::Success,
                    badge_tone(
                        tokens.colors.success.with_alpha(26),
                        tokens.colors.success.with_alpha(80),
                        tokens.colors.success,
                    ),
                ),
                (
                    BadgeTone::Warning,
                    badge_tone(
                        tokens.colors.warning.with_alpha(26),
                        tokens.colors.warning.with_alpha(80),
                        tokens.colors.warning,
                    ),
                ),
                (
                    BadgeTone::Error,
                    badge_tone(
                        tokens.colors.error.with_alpha(26),
                        tokens.colors.error.with_alpha(80),
                        tokens.colors.error,
                    ),
                ),
                (
                    BadgeTone::Blue,
                    badge_tone(
                        tokens.colors.info.with_alpha(26),
                        tokens.colors.info.with_alpha(80),
                        tokens.colors.info,
                    ),
                ),
                (
                    BadgeTone::Orange,
                    badge_tone(
                        tokens.colors.warning.with_alpha(26),
                        tokens.colors.warning.with_alpha(80),
                        tokens.colors.warning,
                    ),
                ),
            ],
        }
    }

    pub fn resolve(&self, tone: BadgeTone, size: ComponentSize) -> ResolvedComponentStyle {
        let base = ResolvedComponentStyle {
            radius: Some(self.radius),
            font_size: Some(self.font_size),
            font_weight: Some(self.font_weight),
            ..ResolvedComponentStyle::default()
        };
        let size_style = find_size_style(&self.sizes, size);
        let tone_style = self
            .tones
            .iter()
            .find(|(candidate, _)| *candidate == tone)
            .map(|(_, style)| style.clone())
            .or_else(|| {
                self.tones
                    .iter()
                    .find(|(candidate, _)| *candidate == BadgeTone::Brand)
                    .map(|(_, style)| style.clone())
            })
            .unwrap_or_default();
        base.merge(&size_style).merge(&tone_style)
    }
}

/// Visual parameters for the `Tabs` widget.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TabsTheme {
    pub active_color: Color,
    pub inactive_color: Color,
    pub indicator_height: f32,
    pub background: Color,
    pub divider_color: Color,
    pub sizes: Vec<(ComponentSize, ResolvedComponentStyle)>,
    pub states: ComponentStateStyles,
    pub track_style: ResolvedComponentStyle,
}

impl TabsTheme {
    pub fn from_tokens(tokens: &Tokens) -> Self {
        Self {
            active_color: tokens.colors.primary,
            inactive_color: tokens.colors.text_secondary,
            indicator_height: 3.0,
            background: tokens.colors.background,
            divider_color: tokens.colors.border.with_alpha(120),
            sizes: vec![
                (
                    ComponentSize::Sm,
                    ResolvedComponentStyle {
                        padding_y: Some(10.0),
                        font_size: Some(tokens.typography.font_size_base),
                        line_height: Some(20.0),
                        height: Some(40.0),
                        ..ResolvedComponentStyle::default()
                    },
                ),
                (
                    ComponentSize::Md,
                    ResolvedComponentStyle {
                        padding_y: Some(12.0),
                        font_size: Some(tokens.typography.font_size_base),
                        line_height: Some(20.0),
                        height: Some(44.0),
                        ..ResolvedComponentStyle::default()
                    },
                ),
            ],
            states: ComponentStateStyles {
                default: ResolvedComponentStyle {
                    text_color: Some(tokens.colors.text_secondary),
                    border: Some(ComponentBorder {
                        fill: Fill::Solid(Color {
                            r: 0,
                            g: 0,
                            b: 0,
                            a: 0,
                        }),
                        width: 2.0,
                    }),
                    ..ResolvedComponentStyle::default()
                },
                hover: Some(ResolvedComponentStyle {
                    text_color: Some(tokens.colors.text_primary),
                    ..ResolvedComponentStyle::default()
                }),
                active: Some(ResolvedComponentStyle {
                    text_color: Some(tokens.colors.primary),
                    border: Some(ComponentBorder {
                        fill: Fill::Solid(tokens.colors.primary),
                        width: 2.0,
                    }),
                    font_weight: Some(tokens.typography.font_weight_semibold),
                    ..ResolvedComponentStyle::default()
                }),
                ..ComponentStateStyles::default()
            },
            track_style: ResolvedComponentStyle {
                background: Some(Fill::Solid(tokens.colors.background)),
                border: Some(ComponentBorder {
                    fill: Fill::Solid(tokens.colors.border.with_alpha(120)),
                    width: 1.0,
                }),
                ..ResolvedComponentStyle::default()
            },
        }
    }

    pub fn resolve_tab(
        &self,
        size: ComponentSize,
        state: ComponentState,
    ) -> ResolvedComponentStyle {
        find_size_style(&self.sizes, size).merge(&self.states.resolve(state))
    }
}

/// Visual parameters for the `Modal` widget.
///
/// Controls the dialog background color, corner radius, shadow, and maximum width.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModalTheme {
    pub bg_color: Color,
    pub radius: f32,
    pub shadow: Option<BoxShadow>,
    pub max_width: f32,
    pub container_style: ResolvedComponentStyle,
    pub scrim_style: ResolvedComponentStyle,
    pub scrim_blur: f32,
}

impl ModalTheme {
    pub fn from_tokens(tokens: &Tokens) -> Self {
        Self {
            bg_color: tokens.colors.surface,
            radius: tokens.radii.large,
            shadow: tokens.elevations.level3,
            max_width: 600.0,
            container_style: ResolvedComponentStyle {
                background: Some(Fill::Solid(tokens.colors.surface)),
                radius: Some(tokens.radii.large),
                max_width: Some(600.0),
                shadows: tokens
                    .elevations
                    .level3
                    .map(shadow_layer_from_box)
                    .into_iter()
                    .collect(),
                ..ResolvedComponentStyle::default()
            },
            scrim_style: ResolvedComponentStyle {
                background: Some(Fill::Solid(Color {
                    r: 15,
                    g: 23,
                    b: 42,
                    a: 153,
                })),
                ..ResolvedComponentStyle::default()
            },
            scrim_blur: 4.0,
        }
    }
}

/// Visual parameters for the `TreeView` widget.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TreeViewTheme {
    pub indent: f32,
    pub selected_bg: Color,
    pub hover_bg: Color,
}

impl TreeViewTheme {
    pub fn from_tokens(tokens: &Tokens) -> Self {
        Self {
            indent: 16.0,
            selected_bg: tokens.colors.primary.with_alpha(52),
            hover_bg: tokens.colors.surface,
        }
    }
}

/// Visual parameters for the `ProgressBar` widget.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProgressTheme {
    pub height: f32,
    pub track_color: Color,
    pub bar_color: Color,
    pub radius: f32,
    pub track_style: ResolvedComponentStyle,
    pub fill_style: ResolvedComponentStyle,
}

impl ProgressTheme {
    pub fn from_tokens(tokens: &Tokens) -> Self {
        Self {
            height: 8.0,
            track_color: tokens.colors.border,
            bar_color: tokens.colors.primary,
            radius: tokens.radii.full,
            track_style: ResolvedComponentStyle {
                height: Some(8.0),
                radius: Some(tokens.radii.full),
                background: Some(Fill::Solid(tokens.colors.border)),
                ..ResolvedComponentStyle::default()
            },
            fill_style: ResolvedComponentStyle {
                height: Some(8.0),
                radius: Some(tokens.radii.full),
                background: Some(Fill::Solid(tokens.colors.primary)),
                ..ResolvedComponentStyle::default()
            },
        }
    }
}

/// Visual parameters for the `Tooltip` widget.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TooltipTheme {
    pub bg_color: Color,
    pub text_color: Color,
    pub radius: f32,
    pub font_size: f32,
    pub padding_x: f32,
    pub padding_y: f32,
    pub max_width: f32,
    pub style: ResolvedComponentStyle,
}

impl TooltipTheme {
    pub fn from_tokens(tokens: &Tokens) -> Self {
        Self {
            bg_color: Color {
                r: 50,
                g: 50,
                b: 50,
                a: 255,
            },
            text_color: Color::WHITE,
            radius: tokens.radii.small,
            font_size: 12.0,
            padding_x: 10.0,
            padding_y: 8.0,
            max_width: 240.0,
            style: ResolvedComponentStyle {
                background: Some(Fill::Solid(Color {
                    r: 50,
                    g: 50,
                    b: 50,
                    a: 255,
                })),
                text_color: Some(Color::WHITE),
                radius: Some(tokens.radii.small),
                font_size: Some(12.0),
                padding_x: Some(10.0),
                padding_y: Some(8.0),
                max_width: Some(240.0),
                shadows: tokens
                    .elevations
                    .level2
                    .map(shadow_layer_from_box)
                    .into_iter()
                    .collect(),
                ..ResolvedComponentStyle::default()
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CardTheme {
    pub padding: f32,
    pub radius: f32,
    pub default_pattern: CardPattern,
    pub patterns: Vec<(CardPattern, ResolvedComponentStyle)>,
    pub hover_style: ResolvedComponentStyle,
}

impl CardTheme {
    pub fn from_tokens(tokens: &Tokens) -> Self {
        let base_border = ComponentBorder {
            fill: Fill::Solid(tokens.colors.border),
            width: 1.0,
        };
        Self {
            padding: tokens.spacing.l,
            radius: tokens.radii.xl,
            default_pattern: CardPattern::Raised,
            patterns: vec![
                (
                    CardPattern::Plain,
                    ResolvedComponentStyle {
                        background: Some(Fill::Solid(tokens.colors.surface)),
                        border: Some(base_border.clone()),
                        radius: Some(tokens.radii.xl),
                        padding_x: Some(tokens.spacing.l),
                        padding_y: Some(tokens.spacing.l),
                        ..ResolvedComponentStyle::default()
                    },
                ),
                (
                    CardPattern::Raised,
                    ResolvedComponentStyle {
                        background: Some(Fill::Solid(tokens.colors.surface)),
                        border: Some(base_border.clone()),
                        radius: Some(tokens.radii.xl),
                        padding_x: Some(tokens.spacing.l),
                        padding_y: Some(tokens.spacing.l),
                        shadows: tokens
                            .elevations
                            .level2
                            .map(shadow_layer_from_box)
                            .into_iter()
                            .collect(),
                        ..ResolvedComponentStyle::default()
                    },
                ),
                (
                    CardPattern::Tinted,
                    ResolvedComponentStyle {
                        background: Some(Fill::Solid(tokens.colors.primary_subtle)),
                        border: Some(ComponentBorder {
                            fill: Fill::Solid(tokens.colors.primary.with_alpha(80)),
                            width: 1.0,
                        }),
                        radius: Some(tokens.radii.xl),
                        padding_x: Some(tokens.spacing.l),
                        padding_y: Some(tokens.spacing.l),
                        ..ResolvedComponentStyle::default()
                    },
                ),
                (
                    CardPattern::Elevated,
                    ResolvedComponentStyle {
                        background: Some(Fill::Solid(tokens.colors.surface)),
                        border: Some(base_border),
                        radius: Some(tokens.radii.xl),
                        padding_x: Some(tokens.spacing.l),
                        padding_y: Some(tokens.spacing.l),
                        shadows: tokens
                            .elevations
                            .level1
                            .map(shadow_layer_from_box)
                            .into_iter()
                            .collect(),
                        ..ResolvedComponentStyle::default()
                    },
                ),
            ],
            hover_style: ResolvedComponentStyle {
                shadows: tokens
                    .elevations
                    .level2
                    .map(shadow_layer_from_box)
                    .into_iter()
                    .collect(),
                ..ResolvedComponentStyle::default()
            },
        }
    }

    pub fn resolve(&self, pattern: CardPattern, hovered: bool) -> ResolvedComponentStyle {
        let base = self
            .patterns
            .iter()
            .find(|(candidate, _)| *candidate == pattern)
            .map(|(_, style)| style.clone())
            .or_else(|| {
                self.patterns
                    .iter()
                    .find(|(candidate, _)| *candidate == self.default_pattern)
                    .map(|(_, style)| style.clone())
            })
            .unwrap_or_default();
        if hovered {
            base.merge(&self.hover_style)
        } else {
            base
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FeatureIconTheme {
    pub sizes: Vec<(ComponentSize, ResolvedComponentStyle)>,
    pub tones: Vec<(FeatureIconTone, ResolvedComponentStyle)>,
    pub shadow: Option<BoxShadow>,
}

impl FeatureIconTheme {
    pub fn from_tokens(tokens: &Tokens) -> Self {
        Self {
            sizes: vec![
                (
                    ComponentSize::Md,
                    ResolvedComponentStyle {
                        width: Some(40.0),
                        height: Some(40.0),
                        radius: Some(tokens.radii.medium),
                        icon_size: Some(20.0),
                        ..ResolvedComponentStyle::default()
                    },
                ),
                (
                    ComponentSize::Lg,
                    ResolvedComponentStyle {
                        width: Some(48.0),
                        height: Some(48.0),
                        radius: Some(10.0),
                        icon_size: Some(24.0),
                        ..ResolvedComponentStyle::default()
                    },
                ),
                (
                    ComponentSize::Xl,
                    ResolvedComponentStyle {
                        width: Some(56.0),
                        height: Some(56.0),
                        radius: Some(12.0),
                        icon_size: Some(28.0),
                        ..ResolvedComponentStyle::default()
                    },
                ),
            ],
            tones: vec![
                (
                    FeatureIconTone::Brand,
                    badge_tone(
                        tokens.colors.primary_subtle,
                        tokens.colors.primary.with_alpha(40),
                        tokens.colors.primary,
                    ),
                ),
                (
                    FeatureIconTone::Gray,
                    badge_tone(
                        tokens.colors.surface_sunken,
                        tokens.colors.border,
                        tokens.colors.text_primary,
                    ),
                ),
                (
                    FeatureIconTone::Blue,
                    badge_tone(
                        tokens.colors.info.with_alpha(26),
                        tokens.colors.info.with_alpha(80),
                        tokens.colors.info,
                    ),
                ),
                (
                    FeatureIconTone::Orange,
                    badge_tone(
                        tokens.colors.warning.with_alpha(26),
                        tokens.colors.warning.with_alpha(80),
                        tokens.colors.warning,
                    ),
                ),
            ],
            shadow: tokens.elevations.level1,
        }
    }
}

fn badge_tone(background: Color, border: Color, text_color: Color) -> ResolvedComponentStyle {
    ResolvedComponentStyle {
        background: Some(Fill::Solid(background)),
        text_color: Some(text_color),
        border: Some(ComponentBorder {
            fill: Fill::Solid(border),
            width: 1.0,
        }),
        ..ResolvedComponentStyle::default()
    }
}

fn find_size_style(
    styles: &[(ComponentSize, ResolvedComponentStyle)],
    size: ComponentSize,
) -> ResolvedComponentStyle {
    styles
        .iter()
        .find(|(candidate, _)| *candidate == size)
        .map(|(_, style)| style.clone())
        .or_else(|| {
            styles
                .iter()
                .find(|(candidate, _)| *candidate == ComponentSize::Md)
                .map(|(_, style)| style.clone())
        })
        .unwrap_or_default()
}
