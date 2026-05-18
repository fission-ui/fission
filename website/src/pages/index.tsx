import Link from '@docusaurus/Link';
import useBaseUrl from '@docusaurus/useBaseUrl';
import Layout from '@theme/Layout';
import {repoExamples} from '../data/siteContent';
import type {ChartCatalogEntry} from '../data/chartCatalog';
import {featuredChartPreviews} from '../data/chartCatalog';
import styles from './index.module.css';

const overviewSignals = [
  {
    title: 'One shared runtime',
    detail: 'State, reducers, layout, semantics, and rendering stay in one app model.',
    href: '/docs/learn/runtime-model',
    cta: 'See the model',
  },
  {
    title: 'Four real target families',
    detail: 'Desktop, web, Android, and iOS hosts already exist around the same app code.',
    href: '/docs/learn/examples-and-targets',
    cta: 'See targets',
  },
  {
    title: 'Built for verification',
    detail: 'Live tests, diagnostics, semantics, and layout inspection are part of the runtime story.',
    href: '/docs/guides/testing-and-diagnostics',
    cta: 'See testing',
  },
  {
    title: 'Target scaffolding included',
    detail: 'Project setup and host generation are already part of the command-line workflow.',
    href: '/docs/guides/platform-shells-cli-and-testing',
    cta: 'See host setup',
  },
];

const architectureSignals = [
  {
    label: 'State',
    title: 'Plain Rust data stays in charge.',
    detail: 'Product truth is not hidden inside widgets or host callbacks.',
  },
  {
    label: 'Reducers',
    title: 'Every durable change has a named cause.',
    detail: 'Typed actions and reducers keep behavior reviewable and testable.',
  },
  {
    label: 'Host work',
    title: 'Outside work has an explicit path.',
    detail: 'Files, timers, authentication, and services do not leak through rendering.',
  },
  {
    label: 'Render',
    title: 'Layout and paint stay inspectable.',
    detail: 'Tests and diagnostics can inspect structure, semantics, and paint order directly.',
  },
];

const featuredApps = ['counter', 'inbox', 'editor']
  .map((slug) => repoExamples.find((example) => example.slug === slug))
  .filter((example): example is NonNullable<typeof example> => Boolean(example));

const hostTargets = [
  {
    name: 'Desktop',
    summary: 'Fast local loop for reducers, overlays, layout, and diagnostics.',
    command: 'cargo run -p counter',
    href: '/docs/learn/examples-and-targets',
    cta: 'Desktop path',
  },
  {
    name: 'Web',
    summary: 'Browser host path and generated launcher folder around the same app model.',
    command: './examples/web-smoke/platforms/web/run-browser.sh',
    href: '/docs/guides/platform-shells-cli-and-testing',
    cta: 'Web path',
  },
  {
    name: 'Android',
    summary: 'Checked-in emulator path and generated Android host folder.',
    command: './examples/mobile-smoke/platforms/android/run-emulator.sh',
    href: '/docs/guides/platform-shells-cli-and-testing',
    cta: 'Android path',
  },
  {
    name: 'iOS',
    summary: 'Checked-in simulator path and generated iOS host folder.',
    command: './examples/mobile-smoke/platforms/ios/run-sim.sh',
    href: '/docs/guides/platform-shells-cli-and-testing',
    cta: 'iOS path',
  },
];

function Hero() {
  return (
    <header className={styles.hero}>
      <div className='container'>
        <p className={styles.kicker}>Production-ready Rust user interface</p>
        <h1 className={styles.title}>Build desktop, web, Android, and iOS apps in Rust.</h1>
        <p className={styles.subtitle}>
          Fission is a cross-platform user interface framework with one shared runtime, explicit state,
          explicit side effects, and a graphics processing unit-backed rendering pipeline.
        </p>
        <p className={styles.heroBody}>
          You write app state as plain Rust data, update it with reducers, and let Fission keep
          layout, input, time, rendering, and platform boundaries consistent across every target.
          That gives beginners a model they can learn once and teams an architecture they can keep
          shipping with.
        </p>
        <div className={styles.ctaRow}>
          <Link className={styles.primaryCta} to='/docs/learn/quickstart'>
            Start with Quickstart
          </Link>
          <Link className={styles.secondaryCta} to='/docs/learn/overview'>
            Read Learn overview
          </Link>
          <Link className={styles.tertiaryCta} to='/reference/overview/overview'>
            Browse Reference
          </Link>
        </div>
        <div className={styles.commandPanel}>
          <div>
            <p className={styles.commandLabel}>Run a real app</p>
            <code>cargo run -p counter</code>
          </div>
          <div>
            <p className={styles.commandLabel}>Create your own project</p>
            <code>fission init my-app</code>
          </div>
        </div>
      </div>
    </header>
  );
}

function Overview() {
  return (
    <section className={styles.section}>
      <div className='container'>
        <div className={styles.sectionHeader}>
          <p className={styles.sectionLead}>What Fission is</p>
          <h2 className={styles.sectionTitle}>A cross-platform Rust framework built for real products.</h2>
          <p className={styles.sectionIntro}>
            Fission keeps state flow, layout, semantics, input routing, and rendering in one runtime,
            while platform shells handle packaging, windows, browser surfaces, lifecycle, and operating-system integration.
          </p>
        </div>

        <div className={styles.proofRail}>
          {overviewSignals.map((item) => (
            <Link key={item.title} className={styles.proofItem} to={item.href}>
              <strong>{item.title}</strong>
              <span>{item.detail}</span>
              <em>{item.cta}</em>
            </Link>
          ))}
        </div>

        <div className={styles.boundaryBand}>
          <div>
            <p className={styles.boundaryLabel}>Shared across every target</p>
            <h3>State, reducers, layout rules, semantics, rendering stages, and testable runtime behavior.</h3>
          </div>
          <div>
            <p className={styles.boundaryLabel}>Owned by each shell</p>
            <h3>Windows, browser surfaces, package shape, lifecycle hooks, and host-specific integration.</h3>
          </div>
        </div>
      </div>
    </section>
  );
}

function Architecture() {
  return (
    <section className={`${styles.section} ${styles.sectionBand}`}>
      <div className='container'>
        <div className={styles.architectureShell}>
          <div className={styles.architectureLead}>
            <p className={styles.sectionLead}>Why the model stays stable</p>
            <h2 className={styles.sectionTitle}>The important boundaries stay visible.</h2>
            <p className={styles.sectionIntro}>
              Fission is strict about where state changes happen, where host work starts, and how rendering is produced.
            </p>
          </div>

          <div className={styles.flowRail}>
            {architectureSignals.map((item) => (
              <article key={item.label} className={styles.flowStep}>
                <p className={styles.flowLabel}>{item.label}</p>
                <div className={styles.flowCopy}>
                  <h3>{item.title}</h3>
                  <p>{item.detail}</p>
                </div>
              </article>
            ))}
          </div>
        </div>
      </div>
    </section>
  );
}

function ChartShot({chart, index}: {chart: ChartCatalogEntry; index: number}) {
  const imageSrc = useBaseUrl(chart.image);
  return (
    <Link
      className={`${styles.chartShot} ${index === 0 ? styles.chartShotLarge : ''}`}
      to='/docs/charts/catalog'>
      <img src={imageSrc} alt={`${chart.title} chart screenshot`} />
      <span>{chart.title}</span>
    </Link>
  );
}

function Charts() {
  return (
    <section className={`${styles.section} ${styles.chartSection}`}>
      <div className='container'>
        <div className={styles.chartShowcase}>
          <div className={styles.chartCopy}>
            <p className={styles.sectionLead}>Beautiful charts</p>
            <h2 className={styles.sectionTitle}>Dashboards, analytics, finance, maps, networks, and 3D-ready visuals.</h2>
            <p className={styles.sectionIntro}>
              Fission Charts is the native charting layer for Fission apps, with more than 400
              renderer-backed variants covering line, bar, area, pie, scatter, heatmap, financial,
              relationship, map, component, dynamic, and 3D chart work without leaving the Rust UI model.
            </p>
            <div className={styles.chartActions}>
              <Link className={styles.primaryCta} to='/docs/charts/overview'>
                Explore Charts
              </Link>
              <Link className={styles.secondaryCta} to='/docs/charts/catalog'>
                Open catalog
              </Link>
            </div>
          </div>
          <div className={styles.chartMosaic} aria-label='Fission chart screenshots'>
            {featuredChartPreviews.map((chart, index) => (
              <ChartShot key={chart.slug} chart={chart} index={index} />
            ))}
          </div>
        </div>
      </div>
    </section>
  );
}

function Targets() {
  return (
    <section className={styles.section}>
      <div className='container'>
        <div className={styles.sectionHeader}>
          <p className={styles.sectionLead}>Targets</p>
          <h2 className={styles.sectionTitle}>Desktop, web, Android, and iOS stay in the same orbit.</h2>
          <p className={styles.sectionIntro}>
            Start on the host that answers your next product question fastest, then keep the shared model intact.
          </p>
        </div>

        <div className={styles.targetRail}>
          {hostTargets.map((target) => (
            <article key={target.name} className={styles.targetLine}>
              <div className={styles.targetMeta}>
                <h3>{target.name}</h3>
                <p>{target.summary}</p>
              </div>
              <code>{target.command}</code>
              <Link className={styles.cardMetaLink} to={target.href}>
                {target.cta}
              </Link>
            </article>
          ))}
        </div>
      </div>
    </section>
  );
}

function Examples() {
  return (
    <section className={styles.section}>
      <div className='container'>
        <div className={styles.sectionHeader}>
          <p className={styles.sectionLead}>Examples</p>
          <h2 className={styles.sectionTitle}>Small loop, real app shell, large custom tool surface.</h2>
          <p className={styles.sectionIntro}>Start where your evaluation needs the most signal.</p>
        </div>

        <div className={styles.showcaseGrid}>
          {featuredApps.map((example) => (
            <article key={example.slug} className={styles.showcaseCard}>
              <div className={styles.showcaseHead}>
                <h3>{example.title}</h3>
                <code>{example.commands[0]}</code>
              </div>
              <p>{example.summary}</p>
              <div className={styles.showcaseTags}>
                {example.features.slice(0, 2).map((feature) => (
                  <span key={feature} className={styles.showcaseTag}>
                    {feature}
                  </span>
                ))}
              </div>
              <div className={styles.linkRow}>
                {example.docsHref && (
                  <Link className={styles.cardCta} to={example.docsHref}>
                    Open guide
                  </Link>
                )}
                {example.referenceHref && (
                  <Link className={styles.cardMetaLink} to={example.referenceHref}>
                    Open reference
                  </Link>
                )}
              </div>
            </article>
          ))}
        </div>
      </div>
    </section>
  );
}

function NextSteps() {
  return (
    <section className={`${styles.section} ${styles.sectionLast}`}>
      <div className='container'>
        <div className={styles.finalStrip}>
          <p className={styles.sectionLead}>Next</p>
          <h2 className={styles.finalTitle}>Run an app, inspect a host, then go deeper where you need detail.</h2>
          <div className={styles.finalActions}>
            <Link className={styles.primaryCta} to='/examples'>
              Run examples
            </Link>
            <Link className={styles.secondaryCta} to='/docs/guides/platform-shells-cli-and-testing'>
              Inspect hosts
            </Link>
            <Link className={styles.tertiaryCta} to='/docs/guides/testing-and-diagnostics'>
              Review testing
            </Link>
          </div>
        </div>
      </div>
    </section>
  );
}

export default function Home() {
  return (
    <Layout
      title='Fission'
      description='Production-ready Rust user interface for desktop, web, Android, and iOS with deterministic architecture, explicit state, and a graphics processing unit-backed rendering pipeline.'>
      <div className={styles.page}>
        <Hero />
        <Overview />
        <Charts />
        <Architecture />
        <Targets />
        <Examples />
        <NextSteps />
      </div>
    </Layout>
  );
}
