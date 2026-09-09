#[cfg(any(test, target_arch = "wasm32"))]
use fission_core::MotionPreference;

#[cfg(target_arch = "wasm32")]
const REDUCED_MOTION_QUERY: &str = "(prefers-reduced-motion: reduce)";

#[cfg(any(test, target_arch = "wasm32"))]
pub(crate) const fn from_reduced(reduced: bool) -> MotionPreference {
    if reduced {
        MotionPreference::Reduced
    } else {
        MotionPreference::Standard
    }
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn browser_query() -> Option<web_sys::MediaQueryList> {
    web_sys::window()?
        .match_media(REDUCED_MOTION_QUERY)
        .ok()
        .flatten()
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn current() -> Option<MotionPreference> {
    browser_query().map(|query| from_reduced(query.matches()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_host_accessibility_state_to_the_public_preference() {
        assert_eq!(from_reduced(false), MotionPreference::Standard);
        assert_eq!(from_reduced(true), MotionPreference::Reduced);
    }
}
