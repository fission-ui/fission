use fission_ir::op::Color;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Opaque 24-bit RGB color used by terminal frame cells.
pub struct TerminalColor {
    /// Red channel in the inclusive range `0..=255`.
    pub r: u8,
    /// Green channel in the inclusive range `0..=255`.
    pub g: u8,
    /// Blue channel in the inclusive range `0..=255`.
    pub b: u8,
}

impl TerminalColor {
    /// Black (`#000000`).
    pub const BLACK: Self = Self { r: 0, g: 0, b: 0 };
    /// White (`#ffffff`).
    pub const WHITE: Self = Self {
        r: 255,
        g: 255,
        b: 255,
    };

    /// Drops the alpha channel from an IR color after surrounding rendering
    /// has resolved compositing.
    pub fn from_ir(color: Color) -> Self {
        Self {
            r: color.r,
            g: color.g,
            b: color.b,
        }
    }

    /// Alpha-composites this foreground over `background` using an 8-bit
    /// opacity where `0` is transparent and `255` is opaque.
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
/// Visual attributes attached to one terminal cell.
pub struct TerminalStyle {
    /// Foreground glyph color.
    pub fg: TerminalColor,
    /// Cell background color.
    pub bg: TerminalColor,
    /// Whether the terminal should request bold/intense text.
    pub bold: bool,
    /// Whether the terminal should request an underline.
    pub underline: bool,
}

impl TerminalStyle {
    /// Creates a normal-weight, non-underlined style with the given colors.
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
/// One character cell in a rendered terminal frame.
pub struct TerminalCell {
    /// Unicode scalar displayed in this cell.
    pub ch: char,
    /// Foreground, background, and emphasis applied to the cell.
    pub style: TerminalStyle,
}

impl TerminalCell {
    /// Creates a space cell carrying `style`.
    pub fn blank(style: TerminalStyle) -> Self {
        Self { ch: ' ', style }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// Complete row-major terminal output for one Fission frame.
pub struct TerminalFrame {
    /// Number of character columns.
    pub width: u16,
    /// Number of character rows.
    pub height: u16,
    /// Row-major cells; the expected length is `width * height`.
    pub cells: Vec<TerminalCell>,
}

impl TerminalFrame {
    /// Allocates a frame filled with blank cells using `style`.
    pub fn new(width: u16, height: u16, style: TerminalStyle) -> Self {
        let len = usize::from(width).saturating_mul(usize::from(height));
        Self {
            width,
            height,
            cells: vec![TerminalCell::blank(style); len],
        }
    }

    /// Replaces every cell with a styled blank.
    pub fn clear(&mut self, style: TerminalStyle) {
        for cell in &mut self.cells {
            *cell = TerminalCell::blank(style);
        }
    }

    /// Returns the cell at zero-based column `x` and row `y`, or `None` when
    /// the coordinate is outside the frame.
    pub fn get(&self, x: u16, y: u16) -> Option<&TerminalCell> {
        self.index(x, y).and_then(|idx| self.cells.get(idx))
    }

    /// Writes a character and style, clipping coordinates outside the frame.
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

    /// Fills the clipped rectangle with styled blank cells.
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

    /// Draws a clipped horizontal run of `ch` beginning at `(x, y)`.
    pub fn draw_hline(&mut self, x: i32, y: i32, width: i32, ch: char, style: TerminalStyle) {
        if width <= 0 {
            return;
        }
        for col in x..x + width {
            self.set(col, y, ch, style);
        }
    }

    /// Draws a clipped vertical run of `ch` beginning at `(x, y)`.
    pub fn draw_vline(&mut self, x: i32, y: i32, height: i32, ch: char, style: TerminalStyle) {
        if height <= 0 {
            return;
        }
        for row in y..y + height {
            self.set(x, row, ch, style);
        }
    }

    /// Returns the character grid as newline-separated rows without ANSI
    /// styling. Trailing spaces are retained for deterministic snapshots.
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
