import Link from '@docusaurus/Link';
import Layout from '@theme/Layout';
import styles from './experience.module.css';

const examples = [
  {
    title: 'Counter',
    summary:
      'One action at a time, one clear state transition. Best for understanding how reducers, build, and render stay in sync.',
    doc: '/docs/tutorials/counter',
  },
  {
    title: 'Todo',
    summary:
      'A practical step beyond the counter: multiple actions, filtered rendering, and progressive async patterns.',
    doc: '/docs/tutorials/todo',
  },
  {
    title: 'Accessible dashboard',
    summary:
      'A complete app-first path that introduces `TextContent::Key`, locale switching, and semantics for stable UI checks.',
    doc: '/docs/guide/i18n-and-accessibility',
  },
];

export default function Examples() {
  return (
    <Layout title="Examples" description="Explore Fission examples.">
      <main className={`container ${styles.pageShell}`}>
        <section className={styles.section}>
          <h1 className={styles.heading}>Examples</h1>
          <p className={styles.lead}>
            Read examples in order with purpose. Each one reinforces the onboarding flow:
            learn state, keep it deterministic, then move side effects into async boundaries.
          </p>
        </section>
        <section className={styles.section}>
          <div className={styles.grid}>
            {examples.map((item) => (
              <article className={styles.card} key={item.title}>
                <h2>{item.title}</h2>
                <p>{item.summary}</p>
                <Link className={styles.link} to={item.doc}>
                  Start this example
                </Link>
              </article>
            ))}
          </div>
        </section>
      </main>
    </Layout>
  );
}
