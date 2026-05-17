import Link from '@docusaurus/Link';
import Layout from '@theme/Layout';
import {docLanes, proofPoints, repoExamples} from '../data/siteContent';
import styles from './index.module.css';

const featuredExamples = repoExamples.filter((example) =>
  ['counter', 'inbox', 'editor', 'mobile-smoke'].includes(example.slug),
);

const subsystemCards = [
  {
    title: 'App/runtime model',
    detail:
      'The shared model now surfaces `AppState`, `View`, `BuildCtx`, reducers, runtime-owned state, and the widget -> IR -> layout -> paint path.',
    href: '/docs/learn/runtime-model',
    cta: 'Read the model',
  },
  {
    title: 'Resources and capabilities',
    detail:
      'Jobs, services, timers, and typed host capabilities are documented from both the guide and reference angle.',
    href: '/reference/core/resources-and-capabilities',
    cta: 'Open runtime ref',
  },
  {
    title: 'Input, IME, and clipboard',
    detail:
      'The docs now call out the shared input event model, text-edit runtime state, shell clipboard/IME traits, and `text-lab` as the proving ground.',
    href: '/docs/guides/input-events-text-and-env',
    cta: 'See input guide',
  },
  {
    title: 'Portals, media, and 3D',
    detail:
      'Animation requests, portal layers, video/web registrations, and `Scene3D` are mapped to the examples that already use them.',
    href: '/docs/guides/media-animation-portals-and-3d',
    cta: 'See runtime features',
  },
  {
    title: 'Shells, CLI, and tests',
    detail:
      'Desktop/mobile/web shell builders, generated targets, live-driver tests, and diagnostics are tied back to the checked-in smoke flows.',
    href: '/docs/guides/testing-and-diagnostics',
    cta: 'See verification path',
  },
];

function repoHref(path: string) {
  return `https://github.com/worka-ai/fission/tree/main/${path}`;
}

function Hero() {
  return (
    <header className={styles.hero}>
      <div className='container'>
        <p className={styles.kicker}>Deterministic Rust UI, backed by real repo examples</p>
        <h1 className={styles.title}>
          One shared runtime for counters, inboxes, editors, terminals, charts, and target hosts.
        </h1>
        <p className={styles.subtitle}>
          This site now leads with what Fission already proves: typed state and reducers, explicit runtime
          resources, text/IME handling, portals, theming, i18n, tests, and desktop/web/mobile host paths.
        </p>
        <div className={styles.ctaRow}>
          <Link className={styles.primaryCta} to='/docs/learn/overview'>
            Start with Learn
          </Link>
          <Link className={styles.secondaryCta} to='/reference/overview/overview'>
            Browse Reference
          </Link>
          <Link className={styles.tertiaryCta} to='/examples'>
            Inspect repo examples
          </Link>
        </div>
        <div className={styles.commandPanel}>
          <div>
            <p className={styles.commandLabel}>Run something real</p>
            <code>cargo run -p counter</code>
          </div>
          <div>
            <p className={styles.commandLabel}>Scaffold a new app</p>
            <code>fission init my-app</code>
          </div>
        </div>
      </div>
    </header>
  );
}

function ProofStrip() {
  return (
    <section className={styles.section}>
      <div className='container'>
        <div className={styles.statGrid}>
          {proofPoints.map((item) => (
            <article className={styles.statCard} key={item.title}>
              <p className={styles.storyLabel}>{item.title}</p>
              <p className={styles.statCopy}>{item.detail}</p>
            </article>
          ))}
        </div>
      </div>
    </section>
  );
}

function FeaturedExamples() {
  return (
    <section className={styles.section}>
      <div className='container'>
        <p className={styles.sectionLead}>What the repo already proves</p>
        <div className={styles.cardGrid}>
          {featuredExamples.map((example) => (
            <article className={styles.card} key={example.slug}>
              <div className={styles.cardHeader}>
                <p className={styles.storyLabel}>{example.crate}</p>
                <code>{example.commands[0]}</code>
              </div>
              <h2>{example.title}</h2>
              <p>{example.summary}</p>
              <ul className={styles.featureList}>
                {example.features.map((feature) => (
                  <li key={feature}>{feature}</li>
                ))}
              </ul>
              <div className={styles.cardMetaRow}>
                <Link className={styles.cardCta} to='/examples'>
                  View example map
                </Link>
                <Link className={styles.cardMetaLink} to={repoHref(example.repoPath)}>
                  Open repo path
                </Link>
              </div>
            </article>
          ))}
        </div>
      </div>
    </section>
  );
}

function DocLanes() {
  return (
    <section className={styles.section}>
      <div className='container'>
        <p className={styles.sectionLead}>The new documentation lanes</p>
        <div className={styles.cardGrid}>
          {docLanes.map((lane) => (
            <article className={styles.card} key={lane.title}>
              <p className={styles.storyLabel}>{lane.title}</p>
              <h2>{lane.summary}</h2>
              <p>{lane.detail}</p>
              <Link className={styles.cardCta} to={lane.href}>
                Open {lane.title}
              </Link>
            </article>
          ))}
        </div>
      </div>
    </section>
  );
}

function Coverage() {
  return (
    <section className={styles.section}>
      <div className='container'>
        <p className={styles.sectionLead}>What this rewrite now covers directly</p>
        <div className={styles.cardGrid}>
          {subsystemCards.map((item) => (
            <article className={styles.card} key={item.title}>
              <h2>{item.title}</h2>
              <p>{item.detail}</p>
              <Link className={styles.cardCta} to={item.href}>
                {item.cta}
              </Link>
            </article>
          ))}
        </div>
      </div>
    </section>
  );
}

export default function Home() {
  return (
    <Layout
      title='Fission'
      description='Deterministic Rust UI across desktop, web, iOS, and Android, with docs grounded in the real repo surface.'>
      <div className={styles.page}>
        <Hero />
        <ProofStrip />
        <FeaturedExamples />
        <DocLanes />
        <Coverage />
      </div>
    </Layout>
  );
}
