pub const OVERVIEW: &str = "/overview";

#[derive(Clone, Copy)]
pub struct NavItem {
    pub path: &'static str,
    pub label: &'static str,
}

#[derive(Clone, Copy)]
pub struct NavGroup {
    pub title: &'static str,
    pub items: &'static [NavItem],
}

pub const WIDGET_ROUTES: &[NavItem] = &[
    NavItem {
        path: "/widgets/modal",
        label: "Modal",
    },
    NavItem {
        path: "/widgets/drawer",
        label: "Drawer",
    },
    NavItem {
        path: "/widgets/popover",
        label: "Popover",
    },
    NavItem {
        path: "/widgets/tooltip",
        label: "Tooltip",
    },
    NavItem {
        path: "/widgets/toast",
        label: "Toast",
    },
    NavItem {
        path: "/widgets/accordion",
        label: "Accordion",
    },
    NavItem {
        path: "/widgets/tabs",
        label: "Tabs",
    },
    NavItem {
        path: "/widgets/button",
        label: "Button",
    },
    NavItem {
        path: "/widgets/checkbox",
        label: "Checkbox",
    },
    NavItem {
        path: "/widgets/switch",
        label: "Switch",
    },
    NavItem {
        path: "/widgets/sidebar",
        label: "Sidebar",
    },
    NavItem {
        path: "/widgets/carousel",
        label: "Carousel",
    },
];

pub const PROPERTY_ROUTES: &[NavItem] = &[
    NavItem {
        path: "/properties/opacity",
        label: "Opacity",
    },
    NavItem {
        path: "/properties/translate",
        label: "Translate X/Y",
    },
    NavItem {
        path: "/properties/scale",
        label: "Scale",
    },
    NavItem {
        path: "/properties/rotation",
        label: "Rotation",
    },
    NavItem {
        path: "/properties/size",
        label: "Width / Height",
    },
    NavItem {
        path: "/properties/background-color",
        label: "Background Color",
    },
    NavItem {
        path: "/properties/border-color",
        label: "Border Color",
    },
    NavItem {
        path: "/properties/corner-radius",
        label: "Corner Radius",
    },
    NavItem {
        path: "/properties/clip-reveal",
        label: "Clip / Reveal",
    },
];

pub const COMPOSITION_ROUTES: &[NavItem] = &[
    NavItem {
        path: "/composition/additive",
        label: "Additive Modal Motion",
    },
    NavItem {
        path: "/composition/conflict",
        label: "Conflicting Motion",
    },
    NavItem {
        path: "/composition/live-composer",
        label: "Live Composer",
    },
    NavItem {
        path: "/composition/last-wins",
        label: "Ordered Last-Wins",
    },
];

pub const POLICY_ROUTES: &[NavItem] = &[
    NavItem {
        path: "/policy/full",
        label: "Full Motion",
    },
    NavItem {
        path: "/policy/reduced",
        label: "Reduced Motion",
    },
    NavItem {
        path: "/policy/disabled",
        label: "Disabled Motion",
    },
];

pub const DIAGNOSTIC_ROUTES: &[NavItem] = &[
    NavItem {
        path: "/diagnostics/declarations",
        label: "Lowered MotionDeclaration",
    },
    NavItem {
        path: "/diagnostics/expressions",
        label: "Lowered MotionExpr",
    },
    NavItem {
        path: "/diagnostics/timeline",
        label: "Timeline Values",
    },
    NavItem {
        path: "/diagnostics/tests",
        label: "Test Harness Examples",
    },
];

pub const NAV_GROUPS: &[NavGroup] = &[
    NavGroup {
        title: "Widgets",
        items: WIDGET_ROUTES,
    },
    NavGroup {
        title: "Properties",
        items: PROPERTY_ROUTES,
    },
    NavGroup {
        title: "Composition",
        items: COMPOSITION_ROUTES,
    },
    NavGroup {
        title: "Policy",
        items: POLICY_ROUTES,
    },
    NavGroup {
        title: "Diagnostics",
        items: DIAGNOSTIC_ROUTES,
    },
];

pub fn canonical_path(pathname: &str) -> String {
    let mut path = pathname
        .trim()
        .split('?')
        .next()
        .unwrap_or(OVERVIEW)
        .to_string();
    if path.is_empty() || path == "/" {
        return OVERVIEW.to_string();
    }
    while path.len() > 1 && path.ends_with('/') {
        path.pop();
    }
    path
}
