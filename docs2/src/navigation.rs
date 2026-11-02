pub const HOME: &str = "/";
pub const DOCS: &str = "/docs/";
pub const COUNTER_TUTORIAL: &str = "/docs/learn/interactive-counter/";

// These destinations remain owned by the current documentation site until their
// content is migrated into docs2. Keeping them absolute makes that ownership
// explicit and prevents the static-site validator from accepting broken local
// routes during the staged migration.
pub const COMPOSE: &str = "https://fission.rs/compose/";
pub const SHOWCASE: &str = "https://fission.rs/showcase/";
pub const REFERENCE: &str = "https://fission.rs/reference/overview/overview/";
pub const REFERENCE_OVERVIEW: &str = "https://fission.rs/reference/overview/overview/";
pub const INSTALLATION: &str = "https://fission.rs/docs/learn/quickstart/";
pub const FORM_CONTROL: &str = "https://fission.rs/reference/widgets/form-control/";
pub const ROUTER: &str = "https://fission.rs/reference/widgets/router/";
pub const COOKBOOK: &str = "https://fission.rs/docs/cookbook/build-a-counter/";
pub const CHANGELOG: &str = "https://github.com/fission-ui/fission/releases";
pub const GITHUB: &str = "https://github.com/fission-ui/fission";
pub const WIDGETS_GUIDE: &str = "https://fission.rs/docs/guides/layout-and-widgets/";
pub const STATE_GUIDE: &str = "https://fission.rs/docs/guides/state-handles-and-providers/";
pub const TEST_GUIDE: &str = "https://fission.rs/docs/guides/testing-and-diagnostics/";
pub const ACCESSIBILITY_GUIDE: &str = "https://fission.rs/reference/platform/accessibility/";
pub const PACKAGING_GUIDE: &str =
    "https://fission.rs/docs/guides/static-site-packaging-and-docker/";
