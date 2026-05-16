import Link from '@docusaurus/Link';
import Layout from '@theme/Layout';
import styles from './index.module.css';

const storySteps = [
  {
    step: 'Start',
    title: 'Write one Rust app model',
    detail:
      'Model state, actions, and UI in one file. The same structure is used by every runtime target.',
  },
  {
    step: 'Validate',
    title: 'Rebuild quickly and verify behavior',
    detail:
      'Run the generated app, confirm one interaction end-to-end, and lock the reducer behavior before adding complexity.',
  },
  {
    step: 'Expand',
    title: 'Generate targets only when they are needed',
    detail:
      'Add web, iOS, or Android targets and use the generated scripts to validate each target early in your release flow.',
  },
];

const strengths = [
  {
    title: 'One model. Many runtimes.',
    detail:
      'Fission keeps business logic in Rust state and uses generated host projects for platform packaging.',
  },
  {
    title: 'Deterministic behavior',
    detail:
      'Actions and reducers give predictable transitions that are easy to test, replay, and reason about.',
  },
  {
    title: 'Production-oriented iteration',
    detail:
      'Split sync work into reducers, jobs, services, and commands so UI code stays predictable and testable.',
  },
];

function Hero() {
  return (
    <header className={styles.hero}>
      <div className='container'>
        <p className={styles.kicker}>Rust UI framework for products that ship</p>
        <h1 className={styles.title}>
          Build one typed model and ship predictable UI across desktop, web, iOS, and Android.
        </h1>
        <p className={styles.subtitle}>
          Fission gives teams a practical path from first interaction to platform launch: model state and actions first,
          prove behavior in fast local loops, then add targets with confidence.
        </p>
        <div className={styles.ctaRow}>
          <Link className={styles.primaryCta} to='/docs/getting-started/what-is-fission'>
            Start learning Fission
          </Link>
          <Link className={styles.secondaryCta} to='/docs/getting-started/install'>
            Install CLI
          </Link>
          <Link className={styles.tertiaryCta} to='/docs/guide/commands-services-jobs'>
            Learn async boundaries
          </Link>
        </div>
        <p className={styles.quickCommand} aria-label='One command quick start'>
          <span className={styles.quickLabel}>Quick start</span>
          <code>cargo install fission-cli && fission init my-app</code>
        </p>
      </div>
    </header>
  );
}

function Progress() {
  return (
    <section className={styles.section}>
      <div className='container'>
        <p className={styles.sectionLead}>The workflow used for every new team:</p>
        <div className={styles.proofGrid}>
          {storySteps.map((item) => (
            <article className={styles.card} key={item.title}>
              <p className={styles.storyLabel}>{item.step}</p>
              <h2 className={styles.storyTitle}>{item.title}</h2>
              <p className={styles.storyCopy}>{item.detail}</p>
              <Link className={styles.cardCta} to='/docs/guide/playground-driven-workflow'>
                Open workflow
              </Link>
            </article>
          ))}
        </div>
      </div>
    </section>
  );
}

function ProductDifferentiators() {
  return (
    <section className={styles.section}>
      <div className='container'>
        <p className={styles.sectionLead}>What teams choose this approach for:</p>
        <div className={styles.cardGrid}>
          {strengths.map((item) => (
            <article className={styles.card} key={item.title}>
              <h2>{item.title}</h2>
              <p>{item.detail}</p>
              <Link className={styles.cardCta} to='/docs/guide/widgets-and-layout'>
                View related guide
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
      description='Rust UI framework for deterministic, production-oriented apps across desktop, web, iOS, and Android.'>
      <div className={styles.page}>
        <Hero />
        <Progress />
        <ProductDifferentiators />
      </div>
    </Layout>
  );
}
