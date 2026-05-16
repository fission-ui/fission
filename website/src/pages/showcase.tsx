import Layout from '@theme/Layout';
import styles from './experience.module.css';

const stories = [
  {
    title: 'Core playground sample',
    description:
      'A stable sequence for validating reducer determinism, focus order, and render updates with minimal surface area.',
  },
  {
    title: 'Widget gallery',
    description:
      'A practical composition set that demonstrates layout primitives, semantic labels, and component consistency.',
  },
  {
    title: 'Command workflow app',
    description: 'A guided example for async services, background updates, and user-feedback patterns.',
  },
];

export default function Showcase() {
  return (
    <Layout title="Showcase" description="Fission apps and practical implementation stories.">
      <main className={`container ${styles.pageShell}`}>
        <section className={styles.section}>
          <h1 className={styles.heading}>Showcase</h1>
          <p className={styles.lead}>
            We are building a showcase that mirrors a shipping team’s onboarding path: model first, validation second,
            expansion third. Each story is intentionally chosen to answer “when do I use this now?”
          </p>
        </section>
        <section className={styles.section}>
          {stories.map((story) => (
            <article className={styles.card} key={story.title}>
              <h2>{story.title}</h2>
              <p>{story.description}</p>
            </article>
          ))}
        </section>
      </main>
    </Layout>
  );
}
