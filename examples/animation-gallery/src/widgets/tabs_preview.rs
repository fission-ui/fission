use super::common::PreviewShell;
use crate::state::{
    current_composition_atoms, scrub_timeline, AnimationGalleryState, MotionAtom, MotionChoice,
    ScrubTimeline,
};
use fission::build::BuildCtxHandle;
use fission::prelude::*;
use fission::widgets::{TabItem, Tabs, TabsMotion};

pub(super) struct TabsPreview<'a> {
    pub ctx: &'a BuildCtxHandle<AnimationGalleryState>,
    pub state: &'a AnimationGalleryState,
}

impl From<TabsPreview<'_>> for Widget {
    fn from(preview: TabsPreview<'_>) -> Self {
        let progress = if preview.state.playing {
            1.0
        } else {
            preview.state.scrub_ms as f32 / 300.0
        };

        PreviewShell {
            child: Tabs {
                active_index: if progress > 0.5 { 1 } else { 0 },
                items: vec![
                    TabItem {
                        title: "API".into(),
                        content: Text::new("Ergonomic motion").into(),
                        on_press: Some(
                            preview
                                .ctx
                                .bind(ScrubTimeline(0), reduce_with!(scrub_timeline)),
                        ),
                        semantics_identifier: Some("animation-gallery.tabs.api".into()),
                    },
                    TabItem {
                        title: "IR".into(),
                        content: Text::new("Lowered MotionExpr").into(),
                        on_press: Some(
                            preview
                                .ctx
                                .bind(ScrubTimeline(300), reduce_with!(scrub_timeline)),
                        ),
                        semantics_identifier: Some("animation-gallery.tabs.ir".into()),
                    },
                ],
                size: ComponentSize::Sm,
                motion: match preview.state.motion {
                    MotionChoice::None => None,
                    MotionChoice::Composition => Some(
                        compose_tabs_motion(current_composition_atoms(preview.state))
                            .unwrap_or(TabsMotion::Indicator + TabsMotion::SlideContent),
                    ),
                    _ => Some(TabsMotion::Default),
                },
            }
            .into(),
        }
        .into()
    }
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
