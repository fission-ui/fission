import Link from '@docusaurus/Link';
import Layout from '@theme/Layout';
import styles from './index.module.css';

const principles = [
  {
    title: 'Deterministic behavior',
    body: 'State transitions are explicit, typed, and replayable. Teams can reason about behavior the way they reason about business logic.',
  },
  {
    title: 'One model, many targets',
    body: 'The same widget and state model powers desktop, web, and mobile web targets so teams avoid divergent platform codepaths.',
  },
  {
    title: 'Production-first workflows',
    body: 'Commands, services, and jobs keep async work separate from rendering, making behavior auditable and testable.',
  },
  {
    title: 'Built for wide screens',
    body: 'The layout pipeline and widget model are tuned to keep product UI stable across monitor sizes and device classes.',
  },
];

function Hero() {
  return (
    <section className={styles.hero}>
      <div className='container'>
        <p className={styles.kicker}>Rust-powered UI for shipping product teams</p>
        <h1 className={styles.title}>Build one app once and ship it where users already are.</h1>
        <p className={styles.subtitle}>
          Fission gives you a single, deterministic UI model with first-class async boundaries and
          one runtime discipline across desktop, web, and responsive mobile web workflows.
        </p>
        <div className={styles.ctaRow}>
          <Link className={styles.primaryCta} to='/docs/getting-started/first-app'>
            Build your first app
          </Link>
          <Link className={styles.secondaryCta} to='/playground'>
            Open the playground
          </Link>
          <Link className={styles.tertiaryCta} to='/reference/overview/'>
            Browse reference
          </Link>
        </div>
      </div>
    </section>
  );
}

function ProblemSolution() {
  return (
    <section className={styles.section}>
      <div className='container'>
        <div className={styles.storyPanel}>
          <p className={styles.storyLabel}>When products scale</p>
          <p className={styles.storyTitle}>Teams need velocity without sacrificing reliability.</p>
          <p className={styles.storyCopy}>
            You want fast interface changes, predictable releases, and confidence that complex
            interactions stay stable across targets. Fission addresses this by treating UI state,
            rendering, and async behavior as one coherent system, not separate stacks connected by hacks.
          </p>
          <p className={styles.storyCopy}>
            The result is fewer context switches between “frontend” and “runtime plumbing” work, and a shorter
            path from idea to validated behavior.
          </p>
        </div>

        <div className={styles.cardGrid}>
          {principles.map((principle) => (
            <article className={styles.card} key={principle.title}>
              <h2>{principle.title}</h2>
              <p>{principle.body}</p>
            </article>
          ))}
        </div>
      </div>
    </section>
  );
}

function ProofStrip() {
  return (
    <section className={styles.section}>
      <div className='container'>
        <p className={styles.sectionLead}>Start with the workflow that gets results fastest.</p>
        <div className={styles.proofGrid}>
          <p>
            <strong>Read</strong> a guided path: <Link to='/docs/guide/widgets-and-layout'>widgets + layout</Link>, then
            <Link to='/docs/guide/playground-driven-workflow'> try it in the browser</Link>.
          </p>
          <p>
            <strong>Validate</strong> behavior: run through reducer-focused checks, then test async boundaries with
            <Link to='/reference/core/commands-services-jobs'> commands, services, and jobs</Link>.
          </p>
          <p>
            <strong>Publish</strong> once behavior is stable: reuse the same model for desktop, web,
            and responsive mobile web while keeping platform details isolated.
          </p>
        </div>
      </div>
    </section>
  );
}

export default function Home() {
  return (
    <Layout
      title='Fission'
      description='Rust UI framework for desktop and web with deterministic state and production-focused workflows.'>
      <div className={styles.page}>
        <Hero />
        <ProblemSolution />
        <ProofStrip />
      </div>
    </Layout>
  );
}
