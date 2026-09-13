/// How tightly the right sidebar packs its cards, from the viewport height.
///
/// Every size that shrinks on short windows is decided here rather than beside
/// each widget, so the three densities stay consistent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SidebarDensity {
    Comfortable,
    Compact,
    UltraCompact,
}

impl SidebarDensity {
    pub(super) fn for_height(height: f32) -> Self {
        if height < 900.0 {
            Self::UltraCompact
        } else if height < 980.0 {
            Self::Compact
        } else {
            Self::Comfortable
        }
    }

    pub(super) fn is_compact(self) -> bool {
        self != Self::Comfortable
    }

    pub(super) fn spacing(self) -> f32 {
        self.pick(10.0, 12.0, 16.0)
    }

    pub(super) fn calendar_cell_size(self) -> f32 {
        self.pick(26.0, 30.0, 32.0)
    }

    pub(super) fn calendar_padding(self) -> f32 {
        self.pick(8.0, 10.0, 12.0)
    }

    pub(super) fn menu_max_height(self) -> f32 {
        self.pick(120.0, 144.0, 200.0)
    }

    pub(super) fn progress_size(self) -> f32 {
        self.pick(34.0, 40.0, 40.0)
    }

    fn pick(self, ultra_compact: f32, compact: f32, comfortable: f32) -> f32 {
        match self {
            Self::UltraCompact => ultra_compact,
            Self::Compact => compact,
            Self::Comfortable => comfortable,
        }
    }
}
