use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ExampleCategory {
    Apps,
    Galleries,
}

impl ExampleCategory {
    pub(crate) const ALL: [Self; 2] = [Self::Apps, Self::Galleries];

    pub(crate) const fn translation_key(self) -> &'static str {
        match self {
            Self::Apps => "showcase.category.apps",
            Self::Galleries => "showcase.category.galleries",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Target {
    MacOS,
    Windows,
    Linux,
    Web,
}

impl Target {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::MacOS => "macOS",
            Self::Windows => "Windows",
            Self::Linux => "Linux",
            Self::Web => "Web",
        }
    }

    pub(crate) const fn group(self) -> TargetFilter {
        match self {
            Self::MacOS | Self::Windows | Self::Linux => TargetFilter::Desktop,
            Self::Web => TargetFilter::Web,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum TargetFilter {
    #[default]
    All,
    Desktop,
    Web,
}

impl TargetFilter {
    pub(crate) const ALL: [Self; 3] = [Self::All, Self::Desktop, Self::Web];

    pub(crate) const fn translation_key(self) -> &'static str {
        match self {
            Self::All => "showcase.catalog.filter.all",
            Self::Desktop => "showcase.catalog.filter.desktop",
            Self::Web => "showcase.catalog.filter.web",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ExampleDefinition {
    pub(crate) slug: &'static str,
    pub(crate) package: &'static str,
    pub(crate) title_key: &'static str,
    pub(crate) summary_key: &'static str,
    pub(crate) category: ExampleCategory,
    pub(crate) command: &'static str,
    pub(crate) targets: &'static [Target],
}

impl ExampleDefinition {
    pub(crate) fn source_url(self) -> String {
        format!(
            "https://github.com/fission-ui/fission/tree/main/examples/{}",
            self.slug
        )
    }

    pub(crate) fn supports_filter(self, filter: TargetFilter) -> bool {
        filter == TargetFilter::All || self.targets.iter().any(|target| target.group() == filter)
    }
}

const DESKTOP_WEB: &[Target] = &[Target::MacOS, Target::Windows, Target::Linux, Target::Web];
pub(crate) static EXAMPLES: &[ExampleDefinition] = &[
    ExampleDefinition {
        slug: "inbox",
        package: "inbox",
        title_key: "showcase.example.inbox.title",
        summary_key: "showcase.example.inbox.summary",
        category: ExampleCategory::Apps,
        command: "cargo run -p inbox",
        targets: DESKTOP_WEB,
    },
    ExampleDefinition {
        slug: "todo-design-system",
        package: "todo-design-system",
        title_key: "showcase.example.todo_design.title",
        summary_key: "showcase.example.todo_design.summary",
        category: ExampleCategory::Apps,
        command: "cargo run -p todo-design-system",
        targets: DESKTOP_WEB,
    },
    ExampleDefinition {
        slug: "widget-gallery",
        package: "widget-gallery",
        title_key: "showcase.example.widget_gallery.title",
        summary_key: "showcase.example.widget_gallery.summary",
        category: ExampleCategory::Galleries,
        command: "cargo run -p widget-gallery",
        targets: DESKTOP_WEB,
    },
    ExampleDefinition {
        slug: "animation-gallery",
        package: "animation-gallery",
        title_key: "showcase.example.animation_gallery.title",
        summary_key: "showcase.example.animation_gallery.summary",
        category: ExampleCategory::Galleries,
        command: "cargo run -p animation-gallery",
        targets: DESKTOP_WEB,
    },
    ExampleDefinition {
        slug: "chart-gallery",
        package: "chart-gallery",
        title_key: "showcase.example.chart_gallery.title",
        summary_key: "showcase.example.chart_gallery.summary",
        category: ExampleCategory::Galleries,
        command: "cargo run -p chart-gallery",
        targets: DESKTOP_WEB,
    },
    ExampleDefinition {
        slug: "icons_gallery",
        package: "icons_gallery",
        title_key: "showcase.example.icons.title",
        summary_key: "showcase.example.icons.summary",
        category: ExampleCategory::Galleries,
        command: "cargo run -p icons_gallery",
        targets: DESKTOP_WEB,
    },
    ExampleDefinition {
        slug: "text-lab",
        package: "text-lab",
        title_key: "showcase.example.text_lab.title",
        summary_key: "showcase.example.text_lab.summary",
        category: ExampleCategory::Apps,
        command: "cargo run -p text-lab",
        targets: DESKTOP_WEB,
    },
];

pub(crate) fn example_by_slug(slug: &str) -> ExampleDefinition {
    EXAMPLES
        .iter()
        .copied()
        .find(|example| example.slug == slug)
        .unwrap_or(EXAMPLES[0])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deployed_catalogue_contains_only_self_contained_browser_previews() {
        const EXCLUDED: &[&str] = &[
            "editor",
            "embed-video",
            "embed-webview",
            "field-inspector",
            "pokemon-card-store",
            "product-browser",
            "terminal",
        ];

        assert!(EXAMPLES
            .iter()
            .all(|example| !EXCLUDED.contains(&example.slug)));
    }

    #[test]
    fn deployed_catalogue_contains_only_product_examples() {
        assert_eq!(
            ExampleCategory::ALL,
            [ExampleCategory::Apps, ExampleCategory::Galleries]
        );
        assert!(EXAMPLES.iter().all(|example| matches!(
            example.category,
            ExampleCategory::Apps | ExampleCategory::Galleries
        )));
        assert_eq!(example_by_slug("text-lab").category, ExampleCategory::Apps);
        assert!(!EXAMPLES.iter().any(|example| example.slug == "counter"));
        assert!(EXAMPLES
            .iter()
            .all(|example| example.targets.contains(&Target::Web)));
    }
}
