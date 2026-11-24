//! Link-size regression fixture: only this SVG should be retained.
use std::io::{self, Write};

const CLOSE: &str = fission_icons::material::navigation::close::regular();

fn main() -> io::Result<()> {
    io::stdout().write_all(CLOSE.as_bytes())
}
