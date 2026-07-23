use fission::prelude::*;

pub(super) struct ChartSelectionEmpty;

impl From<ChartSelectionEmpty> for Widget {
    fn from(_empty: ChartSelectionEmpty) -> Self {
        Container::new(Text::new("Select a chart from the gallery").color(Color {
            r: 150,
            g: 150,
            b: 150,
            a: 255,
        }))
        .into()
    }
}
