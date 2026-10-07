//! Stable identities carried through scene processing, picking, and replay.

use serde::{Deserialize, Serialize};

macro_rules! scene_id {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(
            Clone,
            Copy,
            Debug,
            Default,
            PartialEq,
            Eq,
            PartialOrd,
            Ord,
            Hash,
            Serialize,
            Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(pub u64);

        impl $name {
            pub const fn new(value: u64) -> Self {
                Self(value)
            }

            pub const fn get(self) -> u64 {
                self.0
            }
        }
    };
}

scene_id!(SceneId, "Stable identity of a retained scene.");
scene_id!(NodeId, "Stable identity of a retained scene node.");
scene_id!(AssetId, "Stable identity of an entry in an asset bundle.");
scene_id!(
    PresentationId,
    "Stable identity of one game-to-scene presentation."
);
