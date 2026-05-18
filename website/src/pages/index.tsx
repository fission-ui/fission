import Link from '@docusaurus/Link';
import useBaseUrl from '@docusaurus/useBaseUrl';
import Layout from '@theme/Layout';
import {featuredChartPreviews, type ChartCatalogEntry} from '../data/chartCatalog';
import styles from './index.module.css';

type Tone = 'teal' | 'blue' | 'orange' | 'gray';

type Signal = {
  glyph: string;
  tone: Tone;
  title: string;
  detail: string;
  link: string;
  href: string;
};

const signals: Signal[] = [
  {
    glyph: 'runtime',
    tone: 'teal',
    title: 'One shared runtime',
    detail: 'State, reducers, layout, semantics, and rendering stay in one app model.',
    link: 'See the model',
    href: '/docs/learn/runtime-model',
  },
  {
    glyph: 'targets',
    tone: 'blue',
    title: 'Four real target families',
    detail: 'Desktop, web, Android, and iOS hosts already exist around the same app code.',
    link: 'See targets',
    href: '/docs/learn/examples-and-targets',
  },
  {
    glyph: 'verification',
    tone: 'orange',
    title: 'Built for verification',
    detail: 'Live tests, diagnostics, semantics, and layout inspection are part of the runtime story.',
    link: 'See testing',
    href: '/docs/guides/testing-and-diagnostics',
  },
  {
    glyph: 'scaffold',
    tone: 'gray',
    title: 'Target scaffolding included',
    detail: 'Project setup and host generation are already part of the command-line workflow.',
    link: 'See host setup',
    href: '/docs/guides/platform-shells-cli-and-testing',
  },
];

const sharedItems = [
  {glyph: 'stateReducers', label: 'State and reducers'},
  {glyph: 'layoutRules', label: 'Layout rules'},
  {glyph: 'semanticsTree', label: 'Semantics tree'},
  {glyph: 'inputRouting', label: 'Input routing'},
  {glyph: 'renderStages', label: 'Rendering stages'},
  {glyph: 'runtimeTests', label: 'Testable runtime behavior'},
];

const shellItems = [
  {glyph: 'windowsSurfaces', label: 'Windows and surfaces'},
  {glyph: 'browserCanvas', label: 'Browser canvas'},
  {glyph: 'packageShape', label: 'Package shape'},
  {glyph: 'lifecycleHooks', label: 'Lifecycle hooks'},
  {glyph: 'osIntegration', label: 'OS integration'},
  {glyph: 'capabilities', label: 'Capability brokering'},
];

const architectureSteps = [
  {
    n: '01',
    title: 'State',
    headline: 'Plain Rust data stays in charge.',
    detail: 'Product truth is not hidden inside widgets or host callbacks.',
    glyph: 'state',
  },
  {
    n: '02',
    title: 'Reducers',
    headline: 'Every durable change has a named cause.',
    detail: 'Typed actions and reducers keep behavior reviewable and testable.',
    glyph: 'reducers',
  },
  {
    n: '03',
    title: 'Host work',
    headline: 'Outside work has an explicit path.',
    detail: 'Files, timers, authentication, and services do not leak through rendering.',
    glyph: 'hostWork',
  },
  {
    n: '04',
    title: 'Render',
    headline: 'Layout and paint stay inspectable.',
    detail: 'Tests and diagnostics can inspect structure, semantics, and paint order directly.',
    glyph: 'render',
  },
];

const targets = [
  {
    name: 'Desktop',
    glyph: 'desktop',
    status: 'Supported',
    statusKind: 'success',
    command: 'cargo run -p counter',
    summary: 'Fast local loop for reducers, overlays, layout, and diagnostics.',
    platforms: ['macOS', 'Linux', 'Windows'],
    href: '/docs/learn/examples-and-targets',
    cta: 'Desktop path',
  },
  {
    name: 'Web',
    glyph: 'web',
    status: 'Smoke path',
    statusKind: 'gray',
    command: './examples/web-smoke/platforms/web/run-browser.sh',
    summary: 'Browser host path and generated launcher folder around the same app model.',
    platforms: ['WASM'],
    href: '/docs/guides/platform-shells-cli-and-testing',
    cta: 'Web path',
  },
  {
    name: 'Android',
    glyph: 'android',
    status: 'Smoke path',
    statusKind: 'gray',
    command: './examples/mobile-smoke/platforms/android/run-emulator.sh',
    summary: 'Checked-in emulator path and generated Android host folder.',
    platforms: ['Emulator'],
    href: '/docs/guides/platform-shells-cli-and-testing',
    cta: 'Android path',
  },
  {
    name: 'iOS',
    glyph: 'ios',
    status: 'Smoke path',
    statusKind: 'gray',
    command: './examples/mobile-smoke/platforms/ios/run-sim.sh',
    summary: 'Checked-in simulator path and generated iOS host folder.',
    platforms: ['Simulator'],
    href: '/docs/guides/platform-shells-cli-and-testing',
    cta: 'iOS path',
  },
];

const exampleApps = [
  {
    title: 'Counter',
    command: 'cargo run -p counter',
    tag: 'Starter',
    glyph: 'counter',
    summary: 'The smallest complete Fission app loop: plain state, two reducers, a widget tree, and buttons bound with the public prelude macros.',
    features: ['typed actions and reducers', 'single-file starter app'],
    accent: 'teal',
    guide: '/docs/cookbook/build-a-counter',
    reference: '/reference/core/state-system',
  },
  {
    title: 'Inbox',
    command: 'cargo run -p inbox',
    tag: 'Product shell',
    glyph: 'inbox',
    summary: 'A product-like mail app that exercises portals, theme switching, locale switching, routing, and host capabilities in one shell.',
    features: ['translation bundles and locale sync', 'OPEN_URL host capability flow'],
    accent: 'blue',
    guide: '/docs/guides/theming-and-i18n',
    reference: '/reference/core/environment-input-and-ime',
  },
  {
    title: 'Fission Editor',
    command: 'cargo run -p fission-editor -- .',
    tag: 'Custom surface',
    glyph: 'editor',
    summary: 'The deepest example in the repo: custom editing surface, jobs, timers, portals, terminal panel, and extensive live tests.',
    features: ['custom render node path', 'resource-driven jobs and timers'],
    accent: 'orange',
    guide: '/docs/guides/resources-and-async',
    reference: '/reference/widgets/media',
  },
];

const footerColumns = [
  {
    title: 'Learn',
    links: [
      ['Overview', '/docs/learn/overview'],
      ['Quickstart', '/docs/learn/quickstart'],
      ['Runtime model', '/docs/learn/runtime-model'],
    ],
  },
  {
    title: 'Guides',
    links: [
      ['App structure', '/docs/guides/app-structure'],
      ['Resources and async', '/docs/guides/resources-and-async'],
      ['Testing and diagnostics', '/docs/guides/testing-and-diagnostics'],
      ['Theming and i18n', '/docs/guides/theming-and-i18n'],
      ['Platform shells', '/docs/guides/platform-shells-cli-and-testing'],
    ],
  },
  {
    title: 'Charts',
    links: [
      ['Overview', '/docs/charts/overview'],
      ['Catalog', '/docs/charts/catalog'],
      ['Data and interaction', '/docs/charts/data-and-interaction'],
      ['3D and GL', '/docs/charts/three-dimensional-and-gl'],
    ],
  },
  {
    title: 'Cookbook',
    links: [
      ['Build a counter', '/docs/cookbook/build-a-counter'],
      ['Add platform targets', '/docs/cookbook/add-platform-targets'],
      ['Write a live interface test', '/docs/cookbook/write-a-live-ui-test'],
    ],
  },
  {
    title: 'Explore',
    links: [
      ['Reference', '/reference/overview/overview'],
      ['Examples', '/examples'],
      ['Playground', '/playground'],
      ['Showcase', '/showcase'],
      ['GitHub', 'https://github.com/worka-ai/fission'],
    ],
  },
];

function Glyph({name, className = ''}: {name: string; className?: string}) {
  const common = {
    className: `fs-glyph ${className}`.trim(),
    viewBox: '0 0 24 24',
    width: 24,
    height: 24,
    fill: 'none',
    xmlns: 'http://www.w3.org/2000/svg',
    'aria-hidden': true,
  };

  switch (name) {
    case 'arrowForward':
      return (
        <svg {...common}>
          <path d='M5 12h13m-5-5 5 5-5 5' stroke='currentColor' strokeWidth='2' strokeLinecap='round' strokeLinejoin='round' />
        </svg>
      );
    case 'arrowOutward':
      return (
        <svg {...common}>
          <path d='M8 7h9v9M17 7 7 17' stroke='currentColor' strokeWidth='2' strokeLinecap='round' strokeLinejoin='round' />
        </svg>
      );
    case 'copy':
      return (
        <svg {...common}>
          <rect x='8' y='8' width='10' height='10' rx='2' stroke='currentColor' strokeWidth='1.8' />
          <path d='M6 15H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h8a2 2 0 0 1 2 2v1' stroke='currentColor' strokeWidth='1.8' strokeLinecap='round' />
        </svg>
      );
    case 'check':
      return (
        <svg {...common}>
          <path d='m5 12 4 4L19 6' stroke='currentColor' strokeWidth='2.2' strokeLinecap='round' strokeLinejoin='round' />
        </svg>
      );
    case 'terminal':
      return (
        <svg {...common}>
          <rect x='3' y='5' width='18' height='14' rx='2.5' stroke='currentColor' strokeWidth='1.8' />
          <path d='m7 9 3 3-3 3M12 15h5' stroke='currentColor' strokeWidth='1.8' strokeLinecap='round' strokeLinejoin='round' />
        </svg>
      );
    case 'newProject':
      return (
        <svg {...common}>
          <path d='M4 8.5V18a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-7.5a2 2 0 0 0-2-2h-6l-2-3H6a2 2 0 0 0-2 2v1Z' stroke='currentColor' strokeWidth='1.8' strokeLinejoin='round' />
          <path d='M12 12v5m-2.5-2.5h5' stroke='currentColor' strokeWidth='1.8' strokeLinecap='round' />
        </svg>
      );
    case 'github':
      return (
        <svg {...common} fill='currentColor'>
          <path d='M12 .8a11.2 11.2 0 0 0-3.54 21.83c.56.1.76-.24.76-.54v-1.9c-3.1.67-3.76-1.5-3.76-1.5-.5-1.29-1.24-1.63-1.24-1.63-1.02-.7.08-.69.08-.69 1.13.08 1.73 1.17 1.73 1.17 1 .1.87 1.86 2.63 2.1.45.08.86.03 1.17-.08.1-.73.4-1.22.72-1.5-2.47-.28-5.07-1.24-5.07-5.5 0-1.22.44-2.22 1.16-3-.12-.28-.5-1.42.11-2.96 0 0 .95-.3 3.1 1.15A10.7 10.7 0 0 1 12 6.86c.96 0 1.93.13 2.84.38 2.15-1.45 3.1-1.15 3.1-1.15.62 1.54.23 2.68.12 2.96.72.78 1.15 1.78 1.15 3 0 4.27-2.6 5.22-5.08 5.5.4.35.77 1.04.77 2.1v2.44c0 .3.2.65.77.54A11.2 11.2 0 0 0 12 .8Z' />
        </svg>
      );
    case 'blog':
      return (
        <svg {...common}>
          <path d='M6 17.5h.01M5 12a7 7 0 0 1 7 7M5 6a13 13 0 0 1 13 13' stroke='currentColor' strokeWidth='2' strokeLinecap='round' />
        </svg>
      );
    case 'spark':
      return (
        <svg {...common}>
          <path d='M12 3 9.7 9.7 3 12l6.7 2.3L12 21l2.3-6.7L21 12l-6.7-2.3L12 3Z' stroke='currentColor' strokeWidth='1.8' strokeLinejoin='round' />
        </svg>
      );
    case 'quickstart':
      return (
        <svg {...common}>
          <path d='M12 3c2.8 1.8 4.2 4 4.2 6.5 0 3.4-2 5.8-4.2 8.5-2.2-2.7-4.2-5.1-4.2-8.5C7.8 7 9.2 4.8 12 3Z' stroke='currentColor' strokeWidth='1.8' strokeLinejoin='round' />
          <circle cx='12' cy='9' r='1.8' fill='currentColor' />
          <path d='M9 18.5 6.5 21M15 18.5l2.5 2.5' stroke='currentColor' strokeWidth='1.8' strokeLinecap='round' />
        </svg>
      );
    case 'counter':
      return (
        <svg {...common}>
          <rect x='4' y='5' width='16' height='14' rx='3' stroke='currentColor' strokeWidth='1.8' />
          <path d='M8 12h8M12 8v8' stroke='currentColor' strokeWidth='1.8' strokeLinecap='round' />
        </svg>
      );
    case 'inbox':
      return (
        <svg {...common}>
          <path d='M4 13 6.5 5h11L20 13v5a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2v-5Z' stroke='currentColor' strokeWidth='1.8' strokeLinejoin='round' />
          <path d='M4 13h4l1.5 3h5L16 13h4' stroke='currentColor' strokeWidth='1.8' strokeLinecap='round' strokeLinejoin='round' />
        </svg>
      );
    case 'editor':
      return (
        <svg {...common}>
          <rect x='4' y='4' width='16' height='16' rx='2.5' stroke='currentColor' strokeWidth='1.8' />
          <path d='M8 9h8M8 13h5M8 17h7' stroke='currentColor' strokeWidth='1.8' strokeLinecap='round' />
        </svg>
      );
    case 'desktop':
      return (
        <svg {...common}>
          <rect x='3' y='5' width='18' height='12' rx='2' stroke='currentColor' strokeWidth='1.8' />
          <path d='M9 21h6M12 17v4' stroke='currentColor' strokeWidth='1.8' strokeLinecap='round' />
        </svg>
      );
    case 'web':
      return (
        <svg {...common}>
          <circle cx='12' cy='12' r='8.5' stroke='currentColor' strokeWidth='1.8' />
          <path d='M3.5 12h17M12 3.5c2.2 2.3 3.2 5.1 3.2 8.5s-1 6.2-3.2 8.5c-2.2-2.3-3.2-5.1-3.2-8.5S9.8 5.8 12 3.5Z' stroke='currentColor' strokeWidth='1.4' />
        </svg>
      );
    case 'android':
    case 'ios':
      return (
        <svg {...common}>
          <rect x='7' y='3' width='10' height='18' rx='2.5' stroke='currentColor' strokeWidth='1.8' />
          <path d='M10 6h4M11 18h2' stroke='currentColor' strokeWidth='1.8' strokeLinecap='round' />
        </svg>
      );
    case 'targets':
      return (
        <svg {...common}>
          <rect x='3' y='4' width='7' height='6' rx='1.5' stroke='currentColor' strokeWidth='1.7' />
          <rect x='14' y='4' width='7' height='6' rx='1.5' stroke='currentColor' strokeWidth='1.7' />
          <rect x='3' y='14' width='7' height='6' rx='1.5' stroke='currentColor' strokeWidth='1.7' />
          <rect x='14' y='14' width='7' height='6' rx='1.5' stroke='currentColor' strokeWidth='1.7' />
        </svg>
      );
    case 'verification':
    case 'runtimeTests':
      return (
        <svg {...common}>
          <path d='M5 12.5 9.2 16 19 7' stroke='currentColor' strokeWidth='2' strokeLinecap='round' strokeLinejoin='round' />
          <path d='M4 6.5h7M4 18h16' stroke='currentColor' strokeWidth='1.5' strokeLinecap='round' opacity='0.45' />
        </svg>
      );
    case 'scaffold':
    case 'packageShape':
      return (
        <svg {...common}>
          <path d='M12 3 4.5 7.2 12 11.4l7.5-4.2L12 3ZM4.5 11.2 12 15.4l7.5-4.2M4.5 15.2 12 19.4l7.5-4.2' stroke='currentColor' strokeWidth='1.7' strokeLinejoin='round' />
        </svg>
      );
    case 'reducers':
    case 'stateReducers':
      return (
        <svg {...common}>
          <circle cx='6' cy='7' r='2.5' stroke='currentColor' strokeWidth='1.7' />
          <circle cx='18' cy='7' r='2.5' stroke='currentColor' strokeWidth='1.7' />
          <circle cx='12' cy='17' r='2.5' stroke='currentColor' strokeWidth='1.7' />
          <path d='M8.2 8.8 10.8 14M15.8 8.8 13.2 14' stroke='currentColor' strokeWidth='1.7' strokeLinecap='round' />
        </svg>
      );
    case 'layoutRules':
      return (
        <svg {...common}>
          <rect x='4' y='5' width='16' height='14' rx='2' stroke='currentColor' strokeWidth='1.8' />
          <path d='M4 10h16M10 10v9' stroke='currentColor' strokeWidth='1.8' />
        </svg>
      );
    case 'semanticsTree':
      return (
        <svg {...common}>
          <circle cx='12' cy='5' r='2' stroke='currentColor' strokeWidth='1.7' />
          <circle cx='7' cy='17' r='2' stroke='currentColor' strokeWidth='1.7' />
          <circle cx='17' cy='17' r='2' stroke='currentColor' strokeWidth='1.7' />
          <path d='M12 7v4m0 0H7v4m5-4h5v4' stroke='currentColor' strokeWidth='1.7' strokeLinecap='round' />
        </svg>
      );
    case 'inputRouting':
      return (
        <svg {...common}>
          <path d='M5 5v14l4-4 2.5 5 2.3-1.2-2.5-4.8H17L5 5Z' stroke='currentColor' strokeWidth='1.7' strokeLinejoin='round' />
        </svg>
      );
    case 'render':
    case 'renderStages':
      return (
        <svg {...common}>
          <path d='M4 8.5 12 4l8 4.5-8 4.5-8-4.5ZM4 13l8 4.5L20 13' stroke='currentColor' strokeWidth='1.7' strokeLinejoin='round' />
        </svg>
      );
    case 'windowsSurfaces':
      return (
        <svg {...common}>
          <rect x='4' y='5' width='16' height='13' rx='2' stroke='currentColor' strokeWidth='1.8' />
          <path d='M4 9h16' stroke='currentColor' strokeWidth='1.8' />
        </svg>
      );
    case 'browserCanvas':
      return (
        <svg {...common}>
          <rect x='4' y='5' width='16' height='14' rx='2' stroke='currentColor' strokeWidth='1.8' />
          <path d='M4 9h16M8 15h8' stroke='currentColor' strokeWidth='1.8' strokeLinecap='round' />
        </svg>
      );
    case 'lifecycleHooks':
      return (
        <svg {...common}>
          <path d='M7 7a7 7 0 0 1 10 0l1.5 1.5M17 17a7 7 0 0 1-10 0L5.5 15.5' stroke='currentColor' strokeWidth='1.8' strokeLinecap='round' />
          <path d='M18.5 4.5v4h-4M5.5 19.5v-4h4' stroke='currentColor' strokeWidth='1.8' strokeLinecap='round' strokeLinejoin='round' />
        </svg>
      );
    case 'osIntegration':
    case 'capabilities':
    case 'hostWork':
      return (
        <svg {...common}>
          <rect x='4' y='7' width='7' height='10' rx='2' stroke='currentColor' strokeWidth='1.8' />
          <rect x='13' y='7' width='7' height='10' rx='2' stroke='currentColor' strokeWidth='1.8' />
          <path d='M11 12h2' stroke='currentColor' strokeWidth='1.8' strokeLinecap='round' />
        </svg>
      );
    case 'sharedRuntime':
      return (
        <svg {...common}>
          <circle cx='12' cy='12' r='3' fill='currentColor' />
          <path d='M4 12a8 3.8 0 1 0 16 0 8 3.8 0 1 0-16 0ZM12 4a3.8 8 0 1 0 0 16 3.8 8 0 1 0 0-16Z' stroke='currentColor' strokeWidth='1.5' />
        </svg>
      );
    case 'shellOwner':
      return (
        <svg {...common}>
          <path d='M5 6h14v12H5V6Z' stroke='currentColor' strokeWidth='1.8' />
          <path d='M8 9h3v3H8V9ZM13 9h3v3h-3V9ZM8 14h8' stroke='currentColor' strokeWidth='1.5' strokeLinecap='round' strokeLinejoin='round' />
        </svg>
      );
    case 'runtime':
    case 'state':
    default:
      return (
        <svg {...common}>
          <circle cx='12' cy='12' r='2.8' fill='currentColor' />
          <path d='M4 12c2.8-5.3 13.2-5.3 16 0-2.8 5.3-13.2 5.3-16 0Z' stroke='currentColor' strokeWidth='1.6' />
          <path d='M12 4c5.3 2.8 5.3 13.2 0 16-5.3-2.8-5.3-13.2 0-16Z' stroke='currentColor' strokeWidth='1.6' />
        </svg>
      );
  }
}

function Hero() {
  return (
    <header className={styles.hero}>
      <div className={styles.containerCenter}>
        <span className={styles.heroPill}>
          <span className={styles.heroPillDot} />
          Production-ready Rust user interface
        </span>

        <h1 className={styles.heroTitle}>Build desktop, web, Android, and iOS apps in Rust.</h1>

        <p className={styles.heroLead}>
          Fission is a cross-platform user interface framework with one shared runtime,
          explicit state, explicit side effects, and a GPU-backed rendering pipeline.
        </p>

        <p className={styles.heroBody}>
          You write app state as plain Rust data, update it with reducers, and let Fission keep
          layout, input, time, rendering, and platform boundaries consistent across every target.
        </p>

        <div className={styles.heroActions}>
          <Link to='/docs/learn/quickstart' className='fs-btn fs-btn--xl fs-btn--primary'>
            Start with Quickstart
            <Glyph name='arrowForward' />
          </Link>
          <Link to='/docs/learn/overview' className='fs-btn fs-btn--xl fs-btn--secondary-gray'>
            Read Learn overview
          </Link>
          <Link to='/reference/overview/overview' className={`${styles.linkButton} fs-btn fs-btn--xl fs-btn--link-color`}>
            Browse Reference
            <Glyph name='arrowOutward' />
          </Link>
        </div>

        <div className={styles.commandGrid}>
          <CommandCard label='Run a real app' command='cargo run -p counter' glyph='terminal' />
          <CommandCard label='Create your own project' command='fission init my-app' glyph='newProject' />
        </div>

        <div className={styles.trustRow}>
          <span><span className={styles.statusDot} />Rust 1.77+</span>
          <span>·</span>
          <span>MIT licensed</span>
          <span>·</span>
          <span>v0.1.0 alpha</span>
          <span>·</span>
          <span>Renders on Vello + wgpu</span>
        </div>
      </div>
    </header>
  );
}

function CommandCard({label, command, glyph}: {label: string; command: string; glyph: string}) {
  return (
    <div className={styles.commandCard}>
      <div className={styles.commandLabelRow}>
        <Glyph name={glyph} />
        <p>{label}</p>
      </div>
      <div className={styles.commandLine}>
        <span>$</span>
        <code>{command}</code>
        <button
          type='button'
          aria-label={`Copy ${command}`}
          onClick={() => navigator.clipboard?.writeText(command)}>
          <Glyph name='copy' />
        </button>
      </div>
    </div>
  );
}

function Signals() {
  return (
    <section id='learn' className={styles.section}>
      <div className={styles.container}>
        <SectionHeader
          kicker='What Fission is'
          title='A cross-platform Rust framework built for real products.'
          intro='Fission keeps state flow, layout, semantics, input routing, and rendering in one runtime, while platform shells handle packaging, windows, browser surfaces, lifecycle, and operating-system integration.'
          centered
        />

        <div className={styles.signalGrid}>
          {signals.map((signal) => (
            <Link key={signal.title} to={signal.href} className={styles.signalCard}>
              <span className={`fs-feature-icon ${signal.tone === 'teal' ? '' : `fs-feature-icon--${signal.tone}`}`}>
                <Glyph name={signal.glyph} />
              </span>
              <h3>{signal.title}</h3>
              <p>{signal.detail}</p>
              <span className={styles.inlineLink}>
                {signal.link}
                <Glyph name='arrowForward' />
              </span>
            </Link>
          ))}
        </div>
      </div>
    </section>
  );
}

function SharedOwned() {
  return (
    <section className={styles.sharedSection}>
      <div className={styles.container}>
        <div className={styles.boundaryShell}>
          <BoundaryPanel
            tone='teal'
            kicker='Shared across every target'
            glyph='sharedRuntime'
            title='State, reducers, layout rules, semantics, rendering stages, and testable runtime behavior.'
            items={sharedItems}
          />
          <BoundaryPanel
            tone='gray'
            kicker='Owned by each shell'
            glyph='shellOwner'
            title='Windows, browser surfaces, package shape, lifecycle hooks, and host-specific integration.'
            items={shellItems}
          />
          <div className={styles.pipelineStrip}>
            <span className='fs-kicker'>Pipeline</span>
            <div className={styles.pipelineSteps}>
              {['Build', 'Lower', 'Layout', 'Paint', 'Render'].map((step, index, arr) => (
                <span key={step} className={styles.pipelineStepWrap}>
                  <span className={styles.pipelineStep}>{step}</span>
                  {index < arr.length - 1 && <span className={styles.pipelineArrow}>→</span>}
                </span>
              ))}
            </div>
            <span className={styles.pipelineNote}>Same pipeline on every host.</span>
          </div>
        </div>
      </div>
    </section>
  );
}

function BoundaryPanel({
  tone,
  kicker,
  glyph,
  title,
  items,
}: {
  tone: 'teal' | 'gray';
  kicker: string;
  glyph: string;
  title: string;
  items: {glyph: string; label: string}[];
}) {
  return (
    <div className={`${styles.boundaryPanel} ${tone === 'teal' ? styles.boundaryPanelTeal : ''}`}>
      <div className={styles.boundaryIntro}>
        <span className={`fs-feature-icon ${tone === 'gray' ? 'fs-feature-icon--gray' : ''}`}>
          <Glyph name={glyph} />
        </span>
        <span className='fs-kicker'>{kicker}</span>
      </div>
      <h3>{title}</h3>
      <ul className={styles.boundaryList}>
        {items.map((item) => (
          <li key={item.label}>
            <span>
              <Glyph name={item.glyph} />
            </span>
            {item.label}
          </li>
        ))}
      </ul>
    </div>
  );
}

function Charts() {
  return (
    <section id='charts' className={`${styles.section} ${styles.chartsSection}`}>
      <div className={styles.container}>
        <div className={styles.chartsHeader}>
          <div>
            <p className='fs-kicker'>Beautiful charts</p>
            <h2>Dashboards, analytics, finance, maps, networks, and 3D-ready visuals.</h2>
          </div>
          <div>
            <p>
              Fission Charts is the native charting layer for Fission apps, with more than{' '}
              <strong>400 renderer-backed variants</strong> covering line, bar, area, pie, scatter,
              heatmap, financial, relationship, map, component, dynamic, and 3D chart work — without leaving the Rust UI model.
            </p>
            <div className={styles.chartActions}>
              <Link className='fs-btn fs-btn--md fs-btn--primary' to='/docs/charts/overview'>Explore Charts</Link>
              <Link className='fs-btn fs-btn--md fs-btn--secondary-gray' to='/docs/charts/catalog'>Open catalog</Link>
            </div>
          </div>
        </div>

        <div className={styles.chartGrid}>
          {featuredChartPreviews.map((chart) => (
            <ChartCard key={chart.slug} chart={chart} />
          ))}
        </div>

        <div className={styles.chartChips}>
          {['Line', 'Bar', 'Area', 'Pie', 'Scatter', 'Heatmap', 'Financial', 'Relationship', 'Map', 'Component', 'Dynamic', '3D'].map((chip) => (
            <span key={chip} className='fs-badge fs-badge--gray'>{chip}</span>
          ))}
        </div>
      </div>
    </section>
  );
}

function ChartCard({chart}: {chart: ChartCatalogEntry}) {
  return (
    <Link className={styles.chartCard} to='/docs/charts/catalog'>
      <div className={styles.chartImageWrap}>
        <img src={useBaseUrl(chart.image)} alt={`${chart.title} chart screenshot`} />
        {chart.slug.includes('3d') && <span className={styles.glBadge}>3D / GL</span>}
      </div>
      <div>
        <p>{chart.title}</p>
      </div>
    </Link>
  );
}

function ArchitectureFlow() {
  return (
    <section id='guides' className={`${styles.section} ${styles.archSection}`}>
      <div className={styles.container}>
        <div className={styles.archGrid}>
          <div className={styles.archLead}>
            <p className='fs-kicker'>Why the model stays stable</p>
            <h2>The important boundaries stay visible.</h2>
            <p>
              Fission is strict about where state changes happen, where host work starts, and how rendering is produced.
            </p>
            <CodeCard />
            <div className={styles.archActions}>
              <Link to='/docs/learn/runtime-model' className='fs-btn fs-btn--md fs-btn--primary'>Read the model</Link>
              <Link to='/reference/overview/overview' className='fs-btn fs-btn--md fs-btn--secondary-gray'>Browse reference</Link>
            </div>
          </div>

          <div className={styles.archCards}>
            {architectureSteps.map((step) => (
              <article key={step.n} className={styles.archCard}>
                <div className={styles.archCardTop}>
                  <div>
                    <span>{step.n}</span>
                    <em>{step.title}</em>
                  </div>
                  <Glyph name={step.glyph} />
                </div>
                <h3>{step.headline}</h3>
                <p>{step.detail}</p>
              </article>
            ))}
          </div>
        </div>
      </div>
    </section>
  );
}

function CodeCard() {
  return (
    <div className={styles.codeCard} aria-label='Reducer code example'>
      <div className={styles.codeChrome}>
        <span />
        <span />
        <span />
        <em>reducer.rs</em>
      </div>
      <pre>{`fn reduce(state: &mut AppState, action: Action) {
  match action {
    Action::Inc => state.count += 1,
    Action::Reset => state.count = 0,
  }
}`}</pre>
    </div>
  );
}

function TargetRail() {
  return (
    <section id='reference' className={styles.section}>
      <div className={styles.container}>
        <SectionHeader
          kicker='Targets'
          title='Desktop, web, Android, and iOS stay in the same orbit.'
          intro='Start on the host that answers your next product question fastest, then keep the shared model intact.'
          centered
        />

        <div className={styles.targetRail}>
          {targets.map((target, index) => (
            <article key={target.name} className={styles.targetRow}>
              <span className='fs-feature-icon'>
                <Glyph name={target.glyph} />
              </span>
              <div className={styles.targetCopy}>
                <div>
                  <h3>{target.name}</h3>
                  <span className={`fs-badge fs-badge--${target.statusKind}`}>
                    {target.status === 'Supported' && <span className={styles.statusDot} />}
                    {target.status}
                  </span>
                  <span>· {target.platforms.join(' · ')}</span>
                </div>
                <p>{target.summary}</p>
              </div>
              <code><span>$</span>{target.command}</code>
              <Link to={target.href} className='fs-btn fs-btn--md fs-btn--link-color'>
                {target.cta}
                <Glyph name='arrowForward' />
              </Link>
            </article>
          ))}
        </div>
      </div>
    </section>
  );
}

function ExamplesGrid() {
  return (
    <section id='examples' className={`${styles.section} ${styles.examplesSection}`}>
      <div className={styles.container}>
        <SectionHeader
          kicker='Examples'
          title='Small loop, real app shell, large custom tool surface.'
          intro='Start where your evaluation needs the most signal.'
          centered
        />

        <div className={styles.exampleGrid}>
          {exampleApps.map((app) => (
            <article key={app.title} className={styles.exampleCard}>
              <div className={styles.previewWrap}>
                <AppPreview kind={app.title} />
                <span className={`fs-badge fs-badge--${app.accent}`}>{app.tag}</span>
              </div>

              <div className={styles.exampleBody}>
                <div className={styles.exampleTitle}>
                  <Glyph name={app.glyph} />
                  <h3>{app.title}</h3>
                </div>
                <code>{app.command}</code>
                <p>{app.summary}</p>
                <ul>
                  {app.features.map((feature) => (
                    <li key={feature}>
                      <Glyph name='check' />
                      {feature}
                    </li>
                  ))}
                </ul>
                <div className={styles.exampleActions}>
                  <Link to={app.guide} className='fs-btn fs-btn--md fs-btn--primary'>Open guide</Link>
                  <Link to={app.reference} className='fs-btn fs-btn--md fs-btn--secondary-gray'>Reference</Link>
                </div>
              </div>
            </article>
          ))}
        </div>
      </div>
    </section>
  );
}

function AppPreview({kind}: {kind: string}) {
  if (kind === 'Counter') {
    return (
      <svg viewBox='0 0 320 200' width='100%' height='100%' preserveAspectRatio='none' role='img' aria-label='Counter app preview'>
        <rect width='320' height='200' fill='#0F172A' />
        <rect x='16' y='14' width='8' height='8' rx='4' fill='#F08820' />
        <rect x='28' y='14' width='8' height='8' rx='4' fill='#94A3B8' opacity='0.4' />
        <rect x='40' y='14' width='8' height='8' rx='4' fill='#94A3B8' opacity='0.4' />
        <rect x='100' y='70' width='120' height='66' rx='10' fill='#0B1220' stroke='#1E293B' />
        <text x='160' y='115' textAnchor='middle' fontSize='36' fontFamily='monospace' fontWeight='700' fill='#2DD4BF'>42</text>
        <rect x='92' y='150' width='64' height='26' rx='6' fill='transparent' stroke='#334155' />
        <text x='124' y='167' textAnchor='middle' fontSize='10' fontFamily='Inter, sans-serif' fontWeight='600' fill='#94A3B8'>Decrement</text>
        <rect x='164' y='150' width='64' height='26' rx='6' fill='#2DD4BF' />
        <text x='196' y='167' textAnchor='middle' fontSize='10' fontFamily='Inter, sans-serif' fontWeight='600' fill='#022A1F'>Increment</text>
      </svg>
    );
  }

  if (kind === 'Inbox') {
    return (
      <svg viewBox='0 0 320 200' width='100%' height='100%' preserveAspectRatio='none' role='img' aria-label='Inbox app preview'>
        <rect width='320' height='200' fill='#0F172A' />
        <rect x='0' y='0' width='80' height='200' fill='#020617' />
        <circle cx='20' cy='20' r='6' fill='#F08820' />
        {['Inbox', 'Sent', 'Drafts', 'Archive'].map((label, index) => (
          <g key={label}>
            <rect x='12' y={42 + index * 22} width='56' height='18' rx='4' fill={index === 0 ? '#0F766E' : 'transparent'} opacity={index === 0 ? 0.4 : 1} />
            <text x='20' y={54 + index * 22} fontSize='9' fontFamily='Inter, sans-serif' fill={index === 0 ? '#5EEAD4' : '#64748B'} fontWeight={index === 0 ? '600' : '500'}>{label}</text>
          </g>
        ))}
        {[0, 1, 2, 3].map((index) => (
          <g key={index}>
            <rect x='88' y={14 + index * 44} width='220' height='38' rx='6' fill={index === 0 ? '#1E293B' : 'transparent'} />
            <circle cx='100' cy={33 + index * 44} r='6' fill={['#F08820', '#4DA6E0', '#2DD4BF', '#94A3B8'][index]} />
            <rect x='114' y={26 + index * 44} width='90' height='5' rx='2' fill='#CBD5E1' />
            <rect x='114' y={36 + index * 44} width='140' height='4' rx='2' fill='#475569' />
            <text x='296' y={31 + index * 44} textAnchor='end' fontSize='7' fontFamily='monospace' fill='#64748B'>{['09:24', '08:11', 'Yest.', 'Mon'][index]}</text>
          </g>
        ))}
      </svg>
    );
  }

  return (
    <svg viewBox='0 0 320 200' width='100%' height='100%' preserveAspectRatio='none' role='img' aria-label='Editor app preview'>
      <rect width='320' height='200' fill='#0F172A' />
      <rect x='0' y='0' width='320' height='20' fill='#020617' />
      <text x='10' y='14' fontSize='9' fontFamily='Inter, sans-serif' fill='#64748B'>main.rs · src/lib.rs · Cargo.toml</text>
      <rect x='0' y='20' width='28' height='180' fill='#0B1220' />
      {Array.from({length: 10}).map((_, index) => (
        <text key={index} x='22' y={36 + index * 14} textAnchor='end' fontSize='8' fontFamily='monospace' fill='#475569'>{index + 1}</text>
      ))}
      {['fn reducer(state, action) {', '  match action {', '    Action::Inc => count += 1', '    Action::Reset => count = 0', '  }', '}', '', '#[test]', 'fn test_inc() {'].map((line, index) => (
        <text key={index} x='36' y={36 + index * 14} fontSize='8.5' fontFamily='monospace' fill={index < 6 ? '#5EEAD4' : '#F08820'}>{line}</text>
      ))}
      <rect x='0' y='160' width='320' height='40' fill='#020617' />
      <text x='10' y='174' fontSize='8' fontFamily='monospace' fill='#5EEAD4'>$ cargo test</text>
      <text x='10' y='188' fontSize='8' fontFamily='monospace' fill='#94A3B8'>running 12 tests · all passed</text>
    </svg>
  );
}

function LandingFooter() {
  return (
    <>
      <section id='cookbook' className={styles.darkCta}>
        <div className={styles.darkCtaInner}>
          <span className={styles.darkPill}>
            <Glyph name='spark' />
            Next
          </span>
          <h2>Run an app, inspect a host, then go deeper where you need detail.</h2>
          <p>
            The shared runtime is sitting right there. The next product question is one{' '}
            <code>cargo</code> command away.
          </p>
          <div className={styles.darkActions}>
            <Link to='/examples' className='fs-btn fs-btn--xl fs-btn--primary'>Run examples</Link>
            <Link to='/docs/guides/platform-shells-cli-and-testing' className={styles.darkSecondary}>Inspect hosts</Link>
            <Link to='/docs/guides/testing-and-diagnostics' className={styles.darkLink}>
              Review testing
              <Glyph name='arrowForward' />
            </Link>
          </div>
        </div>
      </section>

      <footer className={styles.landingFooter}>
        <div className={styles.footerInner}>
          <div className={styles.footerGrid}>
            <div>
              <div className={styles.footerBrand}>
                <img src={useBaseUrl('/img/fission-mark.svg')} alt='' />
                <span>Fission</span>
              </div>
              <p>A cross-platform, GPU-accelerated user interface framework for Rust. MIT licensed.</p>
              <div className={styles.socialRow}>
                <Link to='https://github.com/worka-ai/fission' aria-label='GitHub'><Glyph name='github' /></Link>
                <Link to='/blog' aria-label='Blog'><Glyph name='blog' /></Link>
                <Link to='/showcase' aria-label='Showcase'><Glyph name='spark' /></Link>
                <Link to='/docs/learn/quickstart' aria-label='Quickstart'><Glyph name='quickstart' /></Link>
              </div>
              <div className={styles.versionNote}><span />main · v0.1.0 alpha</div>
            </div>
            {footerColumns.map((column) => (
              <div key={column.title}>
                <p className={styles.footerTitle}>{column.title}</p>
                <div className={styles.footerLinks}>
                  {column.links.map(([label, href]) => (
                    <Link key={label} to={href}>{label}</Link>
                  ))}
                </div>
              </div>
            ))}
          </div>
          <div className={styles.footerBottom}>
            <span>© 2026 Fission · MIT License</span>
            <span>
              The Fission framework is ready to use today, but some areas are actively under development.
              Widget APIs are expected to remain stable; some runtime or shell APIs may get breaking changes before 1.0.0.
            </span>
          </div>
        </div>
      </footer>
    </>
  );
}

function SectionHeader({
  kicker,
  title,
  intro,
  centered = false,
}: {
  kicker: string;
  title: string;
  intro: string;
  centered?: boolean;
}) {
  return (
    <div className={`${styles.sectionHeader} ${centered ? styles.centered : ''}`}>
      <p className='fs-kicker'>{kicker}</p>
      <h2>{title}</h2>
      <p>{intro}</p>
    </div>
  );
}

export default function Home() {
  return (
    <Layout
      noFooter
      title='Fission'
      description='Production-ready Rust user interface for desktop, web, Android, and iOS with deterministic architecture, explicit state, and a GPU-backed rendering pipeline.'>
      <div className={styles.page}>
        <Hero />
        <Signals />
        <SharedOwned />
        <Charts />
        <ArchitectureFlow />
        <TargetRail />
        <ExamplesGrid />
        <LandingFooter />
      </div>
    </Layout>
  );
}
