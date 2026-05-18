export type DocLane = {
  title: string;
  href: string;
  summary: string;
  detail: string;
};

export type RepoExample = {
  slug: string;
  title: string;
  crate: string;
  repoPath: string;
  summary: string;
  features: string[];
  commands: string[];
  docsHref?: string;
  referenceHref?: string;
  testPath?: string;
  bucket: 'starter' | 'surface' | 'product' | 'target';
};

export type ShowcaseStory = {
  title: string;
  summary: string;
  repoPath: string;
  proofs: string[];
  href: string;
};

export type PlaygroundFlow = {
  title: string;
  summary: string;
  commands: string[];
  followUp?: string;
};

export const docLanes: DocLane[] = [
  {
    title: 'Learn',
    href: '/docs/learn/overview',
    summary: 'Understand the shared app model before you worry about individual widgets.',
    detail:
      'Start with state, actions, reducers, widgets, the rendering pipeline, and how shells host the same runtime on every target.',
  },
  {
    title: 'Guides',
    href: '/docs/guides/app-structure',
    summary: 'Work through the parts of a real app one subsystem at a time.',
    detail:
      'Layout, input, environment data, async work, theming, internationalization, media, targets, testing, and diagnostics all live here.',
  },
  {
    title: 'Cookbook',
    href: '/docs/cookbook/build-a-counter',
    summary: 'Follow task-focused walkthroughs when you want to build something concrete next.',
    detail:
      'Use the cookbook for targeted recipes such as a counter, live interface tests, timer resources, target generation, and host capability calls.',
  },
  {
    title: 'Reference',
    href: '/reference/overview/overview',
    summary: 'Jump to the exact crate, widget, or runtime contract when you already know the shape you need.',
    detail:
      'Reference pages mirror the implementation: core runtime, shells, testing tools, diagnostics, and the widget catalog.',
  },
];

export const proofPoints = [
  {
    title: 'Real apps in repo',
    detail:
      'Counter, inbox, editor, terminal, chart gallery, widget gallery, and smoke hosts all live in `examples/` and exercise the same runtime.',
  },
  {
    title: 'Shared runtime',
    detail:
      '`DesktopApp`, `MobileApp`, and `WebApp` all wrap the same action, reducer, build, layout, and rendering pipeline.',
  },
  {
    title: 'Deterministic tooling',
    detail:
      '`fission-test`, `LiveTestClient`, and `fission-diagnostics` all inspect the same runtime contracts instead of guessing from pixels alone.',
  },
  {
    title: 'Target scaffolding',
    detail:
      '`fission init` and `cargo fission add-target` generate host folders for desktop, web, iOS, and Android around the same shared app code.',
  },
];

export const repoExamples: RepoExample[] = [
  {
    slug: 'counter',
    title: 'Counter',
    crate: 'counter',
    repoPath: 'examples/counter',
    summary:
      'The smallest complete Fission app loop, plus a modal, checkbox, text input, selector, animation requests, and a small custom canvas example.',
    features: [
      'typed actions and reducers',
      'selector-backed view model',
      'modal overlay and animation wiring',
    ],
    commands: ['cargo run -p counter'],
    docsHref: '/docs/learn/runtime-model',
    referenceHref: '/reference/core/state-system',
    testPath: 'examples/counter/tests/live_e2e.rs',
    bucket: 'starter',
  },
  {
    slug: 'widget-gallery',
    title: 'Widget Gallery',
    crate: 'widget-gallery',
    repoPath: 'examples/widget-gallery',
    summary:
      'A broad survey of the built-in widget surface, useful when you want to see how common controls, overlays, and data widgets already behave.',
    features: [
      'layout, input, navigation, and feedback widgets',
      'semantic tree and screenshot checks',
    ],
    commands: ['cargo run -p widget-gallery'],
    docsHref: '/docs/guides/layout-and-widgets',
    referenceHref: '/reference/widgets/catalog',
    testPath: 'examples/widget-gallery/tests/live_e2e.rs',
    bucket: 'surface',
  },
  {
    slug: 'text-lab',
    title: 'Text Lab',
    crate: 'text-lab',
    repoPath: 'examples/text-lab',
    summary:
      'The focused proving ground for text input, comboboxes, menus, modal focus, and input method editor-sensitive flows.',
    features: [
      'single-line and multiline editing',
      'combobox overlay teardown',
      'modal text flow and focus handling',
    ],
    commands: ['cargo run -p text-lab'],
    docsHref: '/docs/guides/input-events-text-and-env',
    referenceHref: '/reference/core/environment-input-and-ime',
    testPath: 'examples/text-lab/tests/live_e2e.rs',
    bucket: 'surface',
  },
  {
    slug: 'animation-gallery',
    title: 'Animation Gallery',
    crate: 'animation-gallery',
    repoPath: 'examples/animation-gallery',
    summary:
      'Shows runtime-owned opacity, translation, scale, rotation, clip, and scroll-linked motion in isolation.',
    features: [
      'stable widget IDs',
      'Transition widget and raw animation requests',
    ],
    commands: ['cargo run -p animation-gallery'],
    docsHref: '/docs/guides/media-animation-portals-and-3d',
    referenceHref: '/reference/core/animations-portals-media',
    testPath: 'examples/animation-gallery/tests/live_e2e.rs',
    bucket: 'surface',
  },
  {
    slug: 'icons-gallery',
    title: 'Icons Gallery',
    crate: 'icons_gallery',
    repoPath: 'examples/icons_gallery',
    summary:
      'Exercises the bundled Material icon set and gives you a fast visual pass over available icon assets.',
    features: ['bundled icon assets', 'gallery-style inspection'],
    commands: ['cargo run -p icons_gallery'],
    docsHref: '/docs/guides/layout-and-widgets',
    referenceHref: '/reference/widgets/display-and-data',
    testPath: 'examples/icons_gallery/tests/live_e2e.rs',
    bucket: 'surface',
  },
  {
    slug: 'terminal',
    title: 'Terminal',
    crate: 'terminal',
    repoPath: 'examples/terminal',
    summary:
      'Proves `TerminalView`, timer-driven polling, and clipboard-sensitive keyboard flows for a long-lived embedded session.',
    features: [
      'long-lived session state',
      'timer resource polling',
      'copy and paste behavior',
    ],
    commands: ['cargo run -p terminal'],
    docsHref: '/docs/guides/resources-and-async',
    referenceHref: '/reference/widgets/media',
    testPath: 'examples/terminal/tests/live_e2e.rs',
    bucket: 'product',
  },
  {
    slug: 'embed-video',
    title: 'Video Embed',
    crate: 'embed-video',
    repoPath: 'examples/embed-video',
    summary:
      'A minimal app that isolates the host-backed video embed contract and its rendered surface placeholder.',
    features: ['video registration', 'bounded embed layout', 'surface display op test'],
    commands: ['cargo run -p embed-video'],
    docsHref: '/docs/guides/media-animation-portals-and-3d',
    referenceHref: '/reference/widgets/video',
    testPath: 'examples/embed-video/tests/embed_video.rs',
    bucket: 'surface',
  },
  {
    slug: 'embed-webview',
    title: 'WebView Embed',
    crate: 'embed-webview',
    repoPath: 'examples/embed-webview',
    summary:
      'A minimal app that proves WebView registration, explicit embed sizing, and deterministic surface rendering.',
    features: ['web state registration', 'explicit embed size', 'surface display op test'],
    commands: ['cargo run -p embed-webview'],
    docsHref: '/docs/guides/media-animation-portals-and-3d',
    referenceHref: '/reference/widgets/web-view',
    testPath: 'examples/embed-webview/tests/embed_webview.rs',
    bucket: 'surface',
  },
  {
    slug: 'embed-3d',
    title: '3D Embed',
    crate: 'embed-3d',
    repoPath: 'examples/embed-3d',
    summary:
      'A minimal app that isolates the Scene3D custom embed path inside ordinary Fission layout.',
    features: ['custom embed payload', 'bounded 3D viewport', 'surface display op test'],
    commands: ['cargo run -p embed-3d'],
    docsHref: '/docs/guides/media-animation-portals-and-3d',
    referenceHref: '/reference/core/animations-portals-media',
    testPath: 'examples/embed-3d/tests/embed_3d.rs',
    bucket: 'surface',
  },
  {
    slug: 'inbox',
    title: 'Inbox',
    crate: 'inbox',
    repoPath: 'examples/inbox',
    summary:
      'A product-like mail app that exercises portals, theme switching, locale switching, routing, and host capabilities in one shell.',
    features: [
      'translation bundles and locale sync',
      'OPEN_URL host capability flow',
      'drawer and overlay-heavy navigation',
    ],
    commands: ['cargo run -p inbox'],
    docsHref: '/docs/guides/theming-and-i18n',
    referenceHref: '/reference/core/resources-and-capabilities',
    testPath: 'examples/inbox/tests/live_e2e.rs',
    bucket: 'product',
  },
  {
    slug: 'editor',
    title: 'Fission Editor',
    crate: 'fission-editor',
    repoPath: 'examples/editor',
    summary:
      'The deepest example in the repo: custom editing surface, jobs, timers, portals, terminal panel, and extensive live tests.',
    features: [
      'custom render node path',
      'resource-driven jobs and timers',
      'command palette, menus, hover interactions, and tabs',
    ],
    commands: ['cargo run -p fission-editor -- .'],
    docsHref: '/docs/guides/resources-and-async',
    referenceHref: '/reference/core/platform-runtime',
    testPath: 'examples/editor/tests/live_e2e.rs',
    bucket: 'product',
  },
  {
    slug: 'chart-gallery',
    title: 'Chart Gallery',
    crate: 'chart-gallery',
    repoPath: 'examples/chart-gallery',
    summary:
      'Combines chart widgets with the current 3D embed path through `Scene3D`.',
    features: ['chart series and dataset surface', 'Scene3D embed example'],
    commands: ['cargo run -p chart-gallery'],
    docsHref: '/docs/charts/overview',
    referenceHref: '/docs/charts/catalog',
    testPath: 'examples/chart-gallery/tests/live_e2e.rs',
    bucket: 'product',
  },
  {
    slug: 'mobile-smoke',
    title: 'Mobile Smoke',
    crate: 'mobile-smoke',
    repoPath: 'examples/mobile-smoke',
    summary:
      'The checked-in host path for Android emulator and iOS simulator packaging and launch around shared app code.',
    features: [
      'shared mobile runtime host',
      'test control port forwarding on Android',
      'generated host-folder reference',
    ],
    commands: [
      'cargo run -p mobile-smoke',
      './examples/mobile-smoke/platforms/ios/run-sim.sh',
      './examples/mobile-smoke/platforms/android/run-emulator.sh',
    ],
    docsHref: '/docs/guides/platform-shells-cli-and-testing',
    referenceHref: '/reference/platform/targets',
    testPath: 'examples/mobile-smoke/README.md',
    bucket: 'target',
  },
  {
    slug: 'web-smoke',
    title: 'Web Smoke',
    crate: 'web-smoke',
    repoPath: 'examples/web-smoke',
    summary:
      'The checked-in WebAssembly and browser host path, plus the clearest reference for generated web launcher folders.',
    features: ['wasm-pack build script', 'browser host bootstrap'],
    commands: [
      'cargo run -p web-smoke',
      './examples/web-smoke/platforms/web/run-browser.sh',
    ],
    docsHref: '/docs/guides/platform-shells-cli-and-testing',
    referenceHref: '/reference/platform/targets',
    testPath: 'examples/web-smoke/README.md',
    bucket: 'target',
  },
];

export const showcaseStories: ShowcaseStory[] = [
  {
    title: 'Inbox proves a full app shell with theme, locale, routing, and host actions',
    summary:
      'Theme mode, locale switching, portals, responsive navigation, and capability-backed host work all show up in one coherent product shell.',
    repoPath: 'examples/inbox',
    proofs: [
      'locale bundles and `Env` sync',
      'portal-heavy app chrome and drawers',
      'typed external-link capability flows',
    ],
    href: '/docs/guides/theming-and-i18n',
  },
  {
    title: 'Fission Editor proves custom rendering can stay inside the same runtime model',
    summary:
      'The editor mixes custom surfaces, timers, menus, hover interactions, tabs, and a terminal panel without opting out of reducers, resources, or the pipeline.',
    repoPath: 'examples/editor',
    proofs: [
      'custom render node path',
      'resource-driven background work',
      'large live test coverage',
    ],
    href: '/docs/guides/resources-and-async',
  },
  {
    title: 'Text Lab proves the hard text-input cases before they disappear into a larger app',
    summary:
      'Comboboxes, modal text flows, focus handling, and input method editor-sensitive behavior are isolated so you can verify them directly.',
    repoPath: 'examples/text-lab',
    proofs: [
      'combobox popup teardown',
      'modal focus reachability',
      'text input live tests',
    ],
    href: '/docs/guides/input-events-text-and-env',
  },
  {
    title: 'Target smoke paths keep web and mobile claims grounded in checked-in code',
    summary:
      'The repo includes browser, Android emulator, and iOS simulator launch paths instead of treating target support as a vague promise.',
    repoPath: 'examples/mobile-smoke',
    proofs: [
      'checked-in host scripts',
      'generated target parity',
      'documented prerequisites and caveats',
    ],
    href: '/docs/guides/platform-shells-cli-and-testing',
  },
  {
    title: 'Web Smoke proves the current browser host path around the shared runtime',
    summary:
      'The same app model can be compiled to WebAssembly, launched through a generated browser folder, and validated as a concrete host path.',
    repoPath: 'examples/web-smoke',
    proofs: [
      'wasm-pack driven browser build',
      'generated web host shape',
      'shared app code with desktop preview',
    ],
    href: '/docs/guides/platform-shells-cli-and-testing',
  },
];

export const playgroundFlows: PlaygroundFlow[] = [
  {
    title: 'Fast shared-runtime loop',
    summary:
      'Start with a local app or checked-in example, change one reducer or widget branch, and verify one visible result before moving on.',
    commands: ['cargo run -p counter', 'cargo run -p widget-gallery', 'cargo run -p text-lab'],
    followUp:
      'On many machines this loop is easiest through the desktop host because rebuilds are fast and packaging is minimal, but the real goal is to answer shared-runtime questions that apply across every target.',
  },
  {
    title: 'Live-driver loop',
    summary:
      'Run a real shell with the test-control server enabled and script it with `LiveTestClient` when overlays, semantics, or screenshots matter.',
    commands: [
      'FISSION_TEST_CONTROL_PORT=48711 cargo run -p text-lab',
      'cargo test -p widget-gallery --test live_e2e -- --ignored',
    ],
    followUp:
      'Use this when you want to verify behavior in a real window, not just in a headless harness.',
  },
  {
    title: 'Target smoke loop',
    summary:
      'Use the checked-in or generated host scripts when browser, Android, or iOS behavior is part of the experiment and you need the real target host to answer the question.',
    commands: [
      './examples/web-smoke/platforms/web/run-browser.sh',
      './examples/mobile-smoke/platforms/ios/run-sim.sh',
      './examples/mobile-smoke/platforms/android/run-emulator.sh',
    ],
    followUp:
      'Keep the target READMEs close. They contain the real prerequisites, current caveats, and target-specific environment variables for the host you are validating.',
  },
];
