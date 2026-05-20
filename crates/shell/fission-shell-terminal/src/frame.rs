use fission_ir::op::Color;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TerminalColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl TerminalColor {
    pub const BLACK: Self = Self { r: 0, g: 0, b: 0 };
    pub const WHITE: Self = Self {
        r: 255,
        g: 255,
        b: 255,
    };

    pub fn from_ir(color: Color) -> Self {
        Self {
            r: color.r,
            g: color.g,
            b: color.b,
        }
    }

    pub fn blend_over(self, background: Self, alpha: u8) -> Self {
        if alpha == 255 {
            return self;
        }
        if alpha == 0 {
            return background;
        }
        let a = alpha as u16;
        let inv = 255u16.saturating_sub(a);
        Self {
            r: ((self.r as u16 * a + background.r as u16 * inv) / 255) as u8,
            g: ((self.g as u16 * a + background.g as u16 * inv) / 255) as u8,
            b: ((self.b as u16 * a + background.b as u16 * inv) / 255) as u8,
        }
    }
}

impl From<Color> for TerminalColor {
    fn from(value: Color) -> Self {
        Self::from_ir(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TerminalStyle {
    pub fg: TerminalColor,
    pub bg: TerminalColor,
    pub bold: bool,
    pub underline: bool,
}

impl TerminalStyle {
    pub fn new(fg: TerminalColor, bg: TerminalColor) -> Self {
        Self {
            fg,
            bg,
            bold: false,
            underline: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TerminalCell {
    pub ch: char,
    pub style: TerminalStyle,
}

impl TerminalCell {
    pub fn blank(style: TerminalStyle) -> Self {
        Self { ch: ' ', style }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TerminalFrame {
    pub width: u16,
    pub height: u16,
    pub cells: Vec<TerminalCell>,
}

impl TerminalFrame {
    pub fn new(width: u16, height: u16, style: TerminalStyle) -> Self {
        let len = usize::from(width).saturating_mul(usize::from(height));
        Self {
            width,
            height,
            cells: vec![TerminalCell::blank(style); len],
        }
    }

    pub fn clear(&mut self, style: TerminalStyle) {
        for cell in &mut self.cells {
            *cell = TerminalCell::blank(style);
        }
    }

    pub fn get(&self, x: u16, y: u16) -> Option<&TerminalCell> {
        self.index(x, y).and_then(|idx| self.cells.get(idx))
    }

    pub fn set(&mut self, x: i32, y: i32, ch: char, style: TerminalStyle) {
        if x < 0 || y < 0 {
            return;
        }
        let x = x as u16;
        let y = y as u16;
        if let Some(idx) = self.index(x, y) {
            if let Some(cell) = self.cells.get_mut(idx) {
                cell.ch = ch;
                cell.style = style;
            }
        }
    }

    pub fn fill_rect(&mut self, x: i32, y: i32, width: i32, height: i32, style: TerminalStyle) {
        if width <= 0 || height <= 0 {
            return;
        }
        let left = x.max(0);
        let top = y.max(0);
        let right = (x + width).min(i32::from(self.width));
        let bottom = (y + height).min(i32::from(self.height));
        for row in top..bottom {
            for col in left..right {
                self.set(col, row, ' ', style);
            }
        }
    }

    pub fn draw_hline(&mut self, x: i32, y: i32, width: i32, ch: char, style: TerminalStyle) {
        if width <= 0 {
            return;
        }
        for col in x..x + width {
            self.set(col, y, ch, style);
        }
    }

    pub fn draw_vline(&mut self, x: i32, y: i32, height: i32, ch: char, style: TerminalStyle) {
        if height <= 0 {
            return;
        }
        for row in y..y + height {
            self.set(x, row, ch, style);
        }
    }

    pub fn as_plain_text(&self) -> String {
        let mut out = String::new();
        for y in 0..self.height {
            for x in 0..self.width {
                out.push(self.get(x, y).map(|cell| cell.ch).unwrap_or(' '));
            }
            if y + 1 != self.height {
                out.push('\n');
            }
        }
        out
    }

    fn index(&self, x: u16, y: u16) -> Option<usize> {
        if x >= self.width || y >= self.height {
            return None;
        }
        Some(usize::from(y) * usize::from(self.width) + usize::from(x))
    }
}
