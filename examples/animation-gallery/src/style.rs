use fission::op::Color as IrColor;

pub const SURFACE: IrColor = color(255, 255, 255, 255);
pub const BACKGROUND: IrColor = color(246, 248, 252, 255);
pub const INK: IrColor = color(16, 24, 48, 255);
pub const MUTED: IrColor = color(89, 100, 126, 255);
pub const BORDER: IrColor = color(220, 225, 235, 255);
pub const BLUE: IrColor = color(38, 92, 255, 255);
pub const TEAL: IrColor = color(15, 160, 172, 255);
pub const VIOLET: IrColor = color(125, 92, 255, 255);
pub const PINK: IrColor = color(230, 64, 160, 255);
pub const CYAN: IrColor = color(91, 207, 224, 255);
pub const SOFT_BLUE: IrColor = color(233, 239, 255, 255);
pub const SOFT_TEAL: IrColor = color(226, 249, 250, 255);
pub const SOFT_VIOLET: IrColor = color(241, 236, 255, 255);

pub const fn color(r: u8, g: u8, b: u8, a: u8) -> IrColor {
    IrColor { r, g, b, a }
}
