use super::common::PreviewShell;
use crate::state::{
    current_composition_atoms, scrub_timeline, AnimationGalleryState, MotionAtom, MotionChoice,
    ScrubTimeline,
};
use fission::prelude::*;
use fission::widgets::{TabItem, Tabs, TabsMotion};

fission_component! {
    pub(super) TabsPreview<AnimationGalleryState> {}
    |_preview, state, cx| {
        let progress = if state.playing {
            1.0
        } else {
            state.scrub_ms as f32 / 300.0
        };
    } =>
        PreviewShell(
            Tabs(
                active_index: if progress > 0.5 { 1_usize } else { 0_usize },
                items: vec![
                    TabItem {
                        title: "API".into(),
                        content: Text::new("Ergonomic motion").into(),
                        on_press: Some(
                            cx.bind(ScrubTimeline(0), reduce_with!(scrub_timeline)),
                        ),
                        semantics_identifier: Some("animation-gallery.tabs.api".into()),
                    },
                    TabItem {
                        title: "IR".into(),
                        content: Text::new("Lowered MotionExpr").into(),
                        on_press: Some(
                            cx.bind(ScrubTimeline(300), reduce_with!(scrub_timeline)),
                        ),
                        semantics_identifier: Some("animation-gallery.tabs.ir".into()),
                    },
                ],
                size: ComponentSize::Sm,
                motion: match state.motion {
                    MotionChoice::None => None,
                    MotionChoice::Composition => Some(
                        compose_tabs_motion(current_composition_atoms(state))
                            .unwrap_or(TabsMotion::Indicator + TabsMotion::SlideContent),
                    ),
                    _ => Some(TabsMotion::Default),
                },
            ),
        )
}

fn compose_tabs_motion(atoms: &[MotionAtom]) -> Option<TabsMotion> {
    let mut motions = atoms.iter().copied().filter_map(|atom| match atom {
        MotionAtom::Indicator => Some(TabsMotion::Indicator),
        MotionAtom::FadeContent => Some(TabsMotion::FadeContent),
        MotionAtom::SlideContent => Some(TabsMotion::SlideContent),
        _ => None,
    });
    let first = motions.next()?;
    Some(motions.fold(first, |acc, motion| acc + motion))
}
