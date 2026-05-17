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
    summary: 'Get the app/runtime model straight before building product UI.',
    detail: 'State, BuildCtx, View, targets, and the widget -> IR -> layout -> paint pipeline.',
  },
  {
    title: 'Guides',
    href: '/docs/guides/app-structure',
    summary: 'Work one subsystem at a time with repo-backed patterns.',
    detail: 'Async resources, input, IME, theming, i18n, media, portals, shells, testing, and diagnostics.',
  },
  {
    title: 'Cookbook',
    href: '/docs/cookbook/build-a-counter',
    summary: 'Start from practical tasks instead of abstract concepts.',
    detail: 'Counter app, capability calls, timer resources, modal text flows, targets, and live UI tests.',
  },
  {
    title: 'Reference',
    href: '/reference/overview/overview',
    summary: 'Jump to the exact subsystem or widget surface you need.',
    detail: 'Core runtime contracts, shells, CLI, diagnostics, and widget pages grouped by job.',
  },
];

export const proofPoints = [
  {
    title: 'Real apps in repo',
    detail: 'Counter, inbox, editor, terminal, chart gallery, widget gallery, and smoke hosts all live in `examples/`.',
  },
  {
    title: 'Shared runtime',
    detail: '`DesktopApp`, `MobileApp`, and `WebApp` wrap the same action/reducer/build pipeline.',
  },
  {
    title: 'Deterministic tooling',
    detail: '`fission-test`, `LiveTestClient`, and `fission-diagnostics` all target the same runtime contracts.',
  },
  {
    title: 'Target scaffolding',
    detail: '`fission init` and `cargo fission add-target` generate host folders for desktop, web, iOS, and Android.',
  },
];

export const repoExamples: RepoExample[] = [
  {
    slug: 'counter',
    title: 'Counter',
    crate: 'counter',
    repoPath: 'examples/counter',
    summary: 'The smallest complete app loop, plus modal, checkbox, text input, and animation requests.',
    features: ['typed actions + reducers', 'selector-backed view model', 'modal overlay and canvas helper'],
    commands: ['cargo run -p counter'],
    docsHref: '/docs/cookbook/build-a-counter',
    referenceHref: '/reference/core/state-system',
    testPath: 'examples/counter/tests/live_e2e.rs',
    bucket: 'starter',
  },
  {
    slug: 'widget-gallery',
    title: 'Widget Gallery',
    crate: 'widget-gallery',
    repoPath: 'examples/widget-gallery',
    summary: 'A broad survey of the built-in widget surface with live tests and a visual audit.',
    features: ['layout, input, navigation, feedback widgets', 'semantic tree and screenshot checks'],
    commands: ['cargo run -p widget-gallery'],
    docsHref: '/examples',
    referenceHref: '/reference/widgets/catalog',
    testPath: 'examples/widget-gallery/tests/live_e2e.rs',
    bucket: 'surface',
  },
  {
    slug: 'text-lab',
    title: 'Text Lab',
    crate: 'text-lab',
    repoPath: 'examples/text-lab',
    summary: 'The focused proving ground for text input, comboboxes, menus, modal focus, and IME-sensitive flows.',
    features: ['single-line + multiline editing', 'combobox overlay teardown', 'modal text flow'],
    commands: ['cargo run -p text-lab'],
    docsHref: '/docs/cookbook/modal-text-flow',
    referenceHref: '/reference/core/environment-input-and-ime',
    testPath: 'examples/text-lab/tests/live_e2e.rs',
    bucket: 'surface',
  },
  {
    slug: 'animation-gallery',
    title: 'Animation Gallery',
    crate: 'animation-gallery',
    repoPath: 'examples/animation-gallery',
    summary: 'Shows runtime-owned opacity, translation, scale, rotation, clip, and scroll-linked motion.',
    features: ['stable widget IDs', 'Transition widget and raw animation requests'],
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
    summary: 'Exercises the bundled Material icon set and the icons reflection surface.',
    features: ['bundled icon assets', 'gallery-style inspection'],
    commands: ['cargo run -p icons_gallery'],
    referenceHref: '/reference/widgets/display-and-data',
    testPath: 'examples/icons_gallery/tests/live_e2e.rs',
    bucket: 'surface',
  },
  {
    slug: 'terminal',
    title: 'Terminal',
    crate: 'terminal',
    repoPath: 'examples/terminal',
    summary: 'Proves `TerminalView`, timer-driven polling, and clipboard-sensitive keyboard flows.',
    features: ['long-lived session state', 'timer resource polling', 'copy/paste behavior'],
    commands: ['cargo run -p terminal'],
    docsHref: '/docs/cookbook/keep-a-timer-or-service-alive',
    referenceHref: '/reference/widgets/media',
    testPath: 'examples/terminal/tests/live_e2e.rs',
    bucket: 'product',
  },
  {
    slug: 'inbox',
    title: 'Inbox',
    crate: 'inbox',
    repoPath: 'examples/inbox',
    summary: 'A product-like mail UI that exercises portals, theme switching, locale switching, and host capabilities.',
    features: ['translation bundles + locale sync', 'OPEN_URL and AUTHENTICATE flows', 'drawer and overlay-heavy shell'],
    commands: ['cargo run -p inbox'],
    docsHref: '/docs/cookbook/theme-and-locale-toggle',
    referenceHref: '/reference/core/resources-and-capabilities',
    testPath: 'examples/inbox/tests/live_e2e.rs',
    bucket: 'product',
  },
  {
    slug: 'editor',
    title: 'Fission Editor',
    crate: 'fission-editor',
    repoPath: 'examples/editor',
    summary: 'The deepest example in the repo: custom editing surface, timers, portals, terminal panel, and extensive live tests.',
    features: ['custom render node path', 'resource-driven jobs and timers', 'command palette, menus, hover UI, and tabs'],
    commands: ['cargo run -p fission-editor -- .'],
    docsHref: '/docs/guides/testing-and-diagnostics',
    referenceHref: '/reference/core/platform-runtime',
    testPath: 'examples/editor/tests/live_e2e.rs',
    bucket: 'product',
  },
  {
    slug: 'chart-gallery',
    title: 'Chart Gallery',
    crate: 'chart-gallery',
    repoPath: 'examples/chart-gallery',
    summary: 'Combines chart widgets with the current 3D embed path through `Scene3D`.',
    features: ['chart series and dataset surface', 'Scene3D embed example'],
    commands: ['cargo run -p chart-gallery'],
    docsHref: '/docs/guides/media-animation-portals-and-3d',
    referenceHref: '/reference/core/animations-portals-media',
    testPath: 'examples/chart-gallery/tests/live_e2e.rs',
    bucket: 'product',
  },
  {
    slug: 'mobile-smoke',
    title: 'Mobile Smoke',
    crate: 'mobile-smoke',
    repoPath: 'examples/mobile-smoke',
    summary: 'The checked-in host path for Android emulator and iOS simulator packaging and launch.',
    features: ['shared mobile runtime host', 'test control port forwarding', 'generated host-folder reference'],
    commands: [
      'cargo run -p mobile-smoke',
      './examples/mobile-smoke/platforms/ios/run-sim.sh',
      './examples/mobile-smoke/platforms/android/run-emulator.sh',
    ],
    docsHref: '/docs/cookbook/add-platform-targets',
    referenceHref: '/reference/platform/targets',
    testPath: 'examples/mobile-smoke/README.md',
    bucket: 'target',
  },
  {
    slug: 'web-smoke',
    title: 'Web Smoke',
    crate: 'web-smoke',
    repoPath: 'examples/web-smoke',
    summary: 'The checked-in wasm/browser smoke path and the best reference for generated web hosts.',
    features: ['wasm-pack build script', 'browser host bootstrap'],
    commands: [
      'cargo run -p web-smoke',
      './examples/web-smoke/platforms/web/run-browser.sh',
    ],
    docsHref: '/docs/cookbook/add-platform-targets',
    referenceHref: '/reference/platform/targets',
    testPath: 'examples/web-smoke/README.md',
    bucket: 'target',
  },
];

export const showcaseStories: ShowcaseStory[] = [
  {
    title: 'Inbox proves product shell concerns, not just widgets',
    summary: 'Theme mode, locale switching, portals, and capability-backed host actions all show up in one app shell.',
    repoPath: 'examples/inbox',
    proofs: ['locale bundles and `Env` sync', 'portal-heavy app chrome', 'typed external-link and auth flows'],
    href: '/examples',
  },
  {
    title: 'Fission Editor proves custom rendering can live inside the same runtime',
    summary: 'The editor mixes custom surfaces, timers, menus, hover UI, tabs, and a terminal panel without opting out of the framework model.',
    repoPath: 'examples/editor',
    proofs: ['custom render node path', 'resource-driven async work', 'large live test coverage'],
    href: '/examples',
  },
  {
    title: 'Text Lab proves the hard text-input cases in isolation',
    summary: 'Before text input disappears into a large product, the repo already isolates combobox, modal, and IME-sensitive behavior.',
    repoPath: 'examples/text-lab',
    proofs: ['combobox popup teardown', 'modal focus reachability', 'text input live tests'],
    href: '/playground',
  },
  {
    title: 'Mobile and web smoke paths keep platform claims grounded',
    summary: 'The repo includes checked-in browser, iOS simulator, and Android emulator flows instead of treating target support as a vague promise.',
    repoPath: 'examples/mobile-smoke',
    proofs: ['host scripts in repo', 'CLI-generated target parity', 'documented prerequisites and caveats'],
    href: '/docs/cookbook/add-platform-targets',
  },
];

export const playgroundFlows: PlaygroundFlow[] = [
  {
    title: 'Tight desktop loop',
    summary: 'Start with a local app or example, change one reducer or widget branch, and verify one visible outcome.',
    commands: ['cargo run -p counter', 'cargo run -p widget-gallery', 'cargo run -p text-lab'],
    followUp: 'Move to target hosts only after the desktop path is stable.',
  },
  {
    title: 'Live-driver loop',
    summary: 'Run a real window with the HTTP control server enabled and script it with `LiveTestClient`.',
    commands: [
      'FISSION_TEST_CONTROL_PORT=48711 cargo run -p text-lab',
      'cargo test -p widget-gallery --test live_e2e -- --ignored',
    ],
    followUp: 'Use this when overlays, semantics, or screenshots matter more than unit-level speed.',
  },
  {
    title: 'Target smoke loop',
    summary: 'Use the checked-in or generated host scripts to prove packaging and launch on browser or mobile.',
    commands: [
      './examples/web-smoke/platforms/web/run-browser.sh',
      './examples/mobile-smoke/platforms/ios/run-sim.sh',
      './examples/mobile-smoke/platforms/android/run-emulator.sh',
    ],
    followUp: 'Keep the target READMEs close; they contain the real prerequisites and caveats.',
  },
];
