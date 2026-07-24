#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ReproScenario {
    Plain,
    PlainImages,
    Motion,
    MotionImages,
    MotionOpacity,
    MotionTranslate,
    StaticOpacity,
}

impl ReproScenario {
    pub(crate) fn from_env() -> Self {
        match std::env::var("FISSION_REPRO_SCENARIO")
            .unwrap_or_else(|_| "motion".to_string())
            .as_str()
        {
            "plain" => Self::Plain,
            "plain-images" => Self::PlainImages,
            "motion-images" => Self::MotionImages,
            "motion-opacity" => Self::MotionOpacity,
            "motion-translate" => Self::MotionTranslate,
            "static-opacity" => Self::StaticOpacity,
            _ => Self::Motion,
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Plain => "plain",
            Self::PlainImages => "plain-images",
            Self::Motion => "motion",
            Self::MotionImages => "motion-images",
            Self::MotionOpacity => "motion-opacity",
            Self::MotionTranslate => "motion-translate",
            Self::StaticOpacity => "static-opacity",
        }
    }

    pub(crate) fn uses_motion(self) -> bool {
        matches!(
            self,
            Self::Motion | Self::MotionImages | Self::MotionOpacity | Self::MotionTranslate
        )
    }

    pub(crate) fn uses_images(self) -> bool {
        matches!(self, Self::PlainImages | Self::MotionImages)
    }
}
