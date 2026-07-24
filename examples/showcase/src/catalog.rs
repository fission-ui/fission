#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ExampleCategory {
    Start,
    Apps,
    Galleries,
    Platform,
    Diagnostics,
}

impl ExampleCategory {
    pub(crate) const ALL: [Self; 5] = [
        Self::Start,
        Self::Apps,
        Self::Galleries,
        Self::Platform,
        Self::Diagnostics,
    ];

    pub(crate) const fn translation_key(self) -> &'static str {
        match self {
            Self::Start => "showcase.category.start",
            Self::Apps => "showcase.category.apps",
            Self::Galleries => "showcase.category.galleries",
            Self::Platform => "showcase.category.platform",
            Self::Diagnostics => "showcase.category.diagnostics",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Target {
    MacOS,
    Windows,
    Linux,
    Web,
    Android,
    Ios,
    Terminal,
    Ssr,
}

impl Target {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::MacOS => "macOS",
            Self::Windows => "Windows",
            Self::Linux => "Linux",
            Self::Web => "Web",
            Self::Android => "Android",
            Self::Ios => "iOS",
            Self::Terminal => "Terminal",
            Self::Ssr => "SSR",
        }
    }

    pub(crate) const fn group(self) -> TargetFilter {
        match self {
            Self::MacOS | Self::Windows | Self::Linux => TargetFilter::Desktop,
            Self::Web | Self::Ssr => TargetFilter::Web,
            Self::Android | Self::Ios => TargetFilter::Mobile,
            Self::Terminal => TargetFilter::Terminal,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum TargetFilter {
    #[default]
    All,
    Desktop,
    Web,
    Mobile,
    Terminal,
}

impl TargetFilter {
    pub(crate) const ALL: [Self; 5] = [
        Self::All,
        Self::Desktop,
        Self::Web,
        Self::Mobile,
        Self::Terminal,
    ];

    pub(crate) const fn translation_key(self) -> &'static str {
        match self {
            Self::All => "showcase.catalog.filter.all",
            Self::Desktop => "showcase.catalog.filter.desktop",
            Self::Web => "showcase.catalog.filter.web",
            Self::Mobile => "showcase.catalog.filter.mobile",
            Self::Terminal => "showcase.catalog.filter.terminal",
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

const DESKTOP: &[Target] = &[Target::MacOS, Target::Windows, Target::Linux];
const CROSS_PLATFORM: &[Target] = &[
    Target::MacOS,
    Target::Windows,
    Target::Linux,
    Target::Web,
    Target::Android,
    Target::Ios,
];

pub(crate) static EXAMPLES: &[ExampleDefinition] = &[
    ExampleDefinition {
        slug: "counter",
        package: "counter",
        title_key: "showcase.example.counter.title",
        summary_key: "showcase.example.counter.summary",
        category: ExampleCategory::Start,
        command: "cargo run -p counter",
        targets: DESKTOP,
    },
    ExampleDefinition {
        slug: "inbox",
        package: "inbox",
        title_key: "showcase.example.inbox.title",
        summary_key: "showcase.example.inbox.summary",
        category: ExampleCategory::Apps,
        command: "cargo run -p inbox",
        targets: DESKTOP,
    },
    ExampleDefinition {
        slug: "editor",
        package: "fission-editor",
        title_key: "showcase.example.editor.title",
        summary_key: "showcase.example.editor.summary",
        category: ExampleCategory::Apps,
        command: "cargo run -p fission-editor",
        targets: DESKTOP,
    },
    ExampleDefinition {
        slug: "product-browser",
        package: "product-browser",
        title_key: "showcase.example.product_browser.title",
        summary_key: "showcase.example.product_browser.summary",
        category: ExampleCategory::Apps,
        command: "cargo run -p product-browser",
        targets: DESKTOP,
    },
    ExampleDefinition {
        slug: "pokemon-card-store",
        package: "pokemon-card-store",
        title_key: "showcase.example.pokemon_store.title",
        summary_key: "showcase.example.pokemon_store.summary",
        category: ExampleCategory::Apps,
        command: "fission server serve --project-dir examples/pokemon-card-store",
        targets: &[Target::Ssr],
    },
    ExampleDefinition {
        slug: "todo-design-system",
        package: "todo-design-system",
        title_key: "showcase.example.todo_design.title",
        summary_key: "showcase.example.todo_design.summary",
        category: ExampleCategory::Apps,
        command: "cargo run -p todo-design-system",
        targets: DESKTOP,
    },
    ExampleDefinition {
        slug: "widget-gallery",
        package: "widget-gallery",
        title_key: "showcase.example.widget_gallery.title",
        summary_key: "showcase.example.widget_gallery.summary",
        category: ExampleCategory::Galleries,
        command: "cargo run -p widget-gallery",
        targets: DESKTOP,
    },
    ExampleDefinition {
        slug: "animation-gallery",
        package: "animation-gallery",
        title_key: "showcase.example.animation_gallery.title",
        summary_key: "showcase.example.animation_gallery.summary",
        category: ExampleCategory::Galleries,
        command: "cargo run -p animation-gallery",
        targets: DESKTOP,
    },
    ExampleDefinition {
        slug: "chart-gallery",
        package: "chart-gallery",
        title_key: "showcase.example.chart_gallery.title",
        summary_key: "showcase.example.chart_gallery.summary",
        category: ExampleCategory::Galleries,
        command: "cargo run -p chart-gallery",
        targets: DESKTOP,
    },
    ExampleDefinition {
        slug: "icons_gallery",
        package: "icons_gallery",
        title_key: "showcase.example.icons.title",
        summary_key: "showcase.example.icons.summary",
        category: ExampleCategory::Galleries,
        command: "cargo run -p icons_gallery",
        targets: DESKTOP,
    },
    ExampleDefinition {
        slug: "text-lab",
        package: "text-lab",
        title_key: "showcase.example.text_lab.title",
        summary_key: "showcase.example.text_lab.summary",
        category: ExampleCategory::Galleries,
        command: "cargo run -p text-lab",
        targets: DESKTOP,
    },
    ExampleDefinition {
        slug: "field-inspector",
        package: "field-inspector",
        title_key: "showcase.example.field_inspector.title",
        summary_key: "showcase.example.field_inspector.summary",
        category: ExampleCategory::Platform,
        command: "fission run --project-dir examples/field-inspector",
        targets: CROSS_PLATFORM,
    },
    ExampleDefinition {
        slug: "web-smoke",
        package: "web-smoke",
        title_key: "showcase.example.web_smoke.title",
        summary_key: "showcase.example.web_smoke.summary",
        category: ExampleCategory::Platform,
        command: "fission run --target web --project-dir examples/web-smoke",
        targets: CROSS_PLATFORM,
    },
    ExampleDefinition {
        slug: "mobile-smoke",
        package: "mobile-smoke",
        title_key: "showcase.example.mobile_smoke.title",
        summary_key: "showcase.example.mobile_smoke.summary",
        category: ExampleCategory::Platform,
        command: "fission run --target android --project-dir examples/mobile-smoke",
        targets: &[Target::Android, Target::Ios],
    },
    ExampleDefinition {
        slug: "terminal",
        package: "terminal",
        title_key: "showcase.example.terminal.title",
        summary_key: "showcase.example.terminal.summary",
        category: ExampleCategory::Platform,
        command: "cargo run -p terminal",
        targets: &[Target::Terminal],
    },
    ExampleDefinition {
        slug: "embed-3d",
        package: "embed-3d",
        title_key: "showcase.example.embed_3d.title",
        summary_key: "showcase.example.embed_3d.summary",
        category: ExampleCategory::Platform,
        command: "cargo run -p embed-3d",
        targets: DESKTOP,
    },
    ExampleDefinition {
        slug: "embed-video",
        package: "embed-video",
        title_key: "showcase.example.embed_video.title",
        summary_key: "showcase.example.embed_video.summary",
        category: ExampleCategory::Platform,
        command: "cargo run -p embed-video",
        targets: DESKTOP,
    },
    ExampleDefinition {
        slug: "embed-webview",
        package: "embed-webview",
        title_key: "showcase.example.embed_webview.title",
        summary_key: "showcase.example.embed_webview.summary",
        category: ExampleCategory::Platform,
        command: "cargo run -p embed-webview",
        targets: DESKTOP,
    },
    ExampleDefinition {
        slug: "motion-memory-repro",
        package: "motion-memory-repro",
        title_key: "showcase.example.motion_memory.title",
        summary_key: "showcase.example.motion_memory.summary",
        category: ExampleCategory::Diagnostics,
        command: "cargo run -p motion-memory-repro",
        targets: DESKTOP,
    },
];

pub(crate) fn example_by_slug(slug: &str) -> ExampleDefinition {
    EXAMPLES
        .iter()
        .copied()
        .find(|example| example.slug == slug)
        .unwrap_or(EXAMPLES[0])
}
use serde::{Deserialize, Serialize};
