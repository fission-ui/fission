#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AxisPointerType {
    Line,
    Shadow,
    Cross,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AxisPointer {
    pub pointer_type: AxisPointerType,
    pub snap: bool,
}

impl Default for AxisPointer {
    fn default() -> Self {
        Self {
            pointer_type: AxisPointerType::Line,
            snap: false,
        }
    }
}

impl AxisPointer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn pointer_type(mut self, pointer_type: AxisPointerType) -> Self {
        self.pointer_type = pointer_type;
        self
    }

    pub fn snap(mut self, snap: bool) -> Self {
        self.snap = snap;
        self
    }
}
