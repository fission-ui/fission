#[derive(Clone, Copy, Debug)]
pub struct UiDensity {
    compact: bool,
}

impl UiDensity {
    pub fn new(compact: bool) -> Self {
        Self { compact }
    }

    pub fn header_height(self) -> f32 {
        if self.compact {
            2.0
        } else {
            4.0
        }
    }

    pub fn shell_gap(self) -> f32 {
        if self.compact {
            0.0
        } else {
            1.0
        }
    }

    pub fn body_gap(self) -> f32 {
        if self.compact {
            1.0
        } else {
            2.0
        }
    }

    pub fn outer_padding(self) -> [f32; 4] {
        if self.compact {
            [1.0, 1.0, 0.0, 0.0]
        } else {
            [2.0, 2.0, 1.0, 1.0]
        }
    }

    pub fn content_padding(self) -> [f32; 4] {
        if self.compact {
            [1.0, 1.0, 0.0, 0.0]
        } else {
            [2.0, 2.0, 1.0, 1.0]
        }
    }

    pub fn sidebar_padding(self) -> [f32; 4] {
        if self.compact {
            [1.0, 1.0, 0.0, 0.0]
        } else {
            [1.0, 1.0, 1.0, 1.0]
        }
    }

    pub fn sidebar_width(self) -> f32 {
        if self.compact {
            20.0
        } else {
            24.0
        }
    }

    pub fn nav_route_height(self) -> f32 {
        if self.compact {
            1.0
        } else {
            4.0
        }
    }

    pub fn nav_gap(self) -> f32 {
        if self.compact {
            0.0
        } else {
            1.0
        }
    }

    pub fn control_height(self) -> f32 {
        if self.compact {
            1.0
        } else {
            3.0
        }
    }

    pub fn control_padding(self) -> [f32; 4] {
        if self.compact {
            [0.0, 0.0, 0.0, 0.0]
        } else {
            [1.0, 1.0, 0.0, 0.0]
        }
    }

    pub fn text_input_height(self) -> f32 {
        if self.compact {
            3.0
        } else {
            5.0
        }
    }

    pub fn text_input_padding(self) -> [f32; 4] {
        if self.compact {
            [0.0, 0.0, 0.0, 0.0]
        } else {
            [1.0, 1.0, 0.0, 0.0]
        }
    }

    pub fn output_log_height(self, panel_height: f32) -> f32 {
        let reserved = if self.compact { 2.0 } else { 3.0 };
        (panel_height - reserved).max(1.0)
    }

    pub fn shell_metrics(self, height: f32) -> ShellMetrics {
        let header_h = self.header_height();
        let padding = self.outer_padding();
        let gap_h = self.shell_gap() * 2.0;
        let reserved_h = header_h + padding[2] + padding[3] + gap_h;
        let work_h = (height - reserved_h).max(if self.compact { 12.0 } else { 16.0 });
        let footer_h = (work_h * 0.5)
            .max(if self.compact { 6.0 } else { 8.0 })
            .min(work_h - if self.compact { 4.0 } else { 6.0 });
        let body_h = (work_h - footer_h).max(if self.compact { 4.0 } else { 6.0 });
        ShellMetrics { body_h, footer_h }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ShellMetrics {
    pub body_h: f32,
    pub footer_h: f32,
}
