import Link from '@docusaurus/Link';
import Layout from '@theme/Layout';
import styles from './experience.module.css';

function DemoCard({title, description, url}: {title: string; description: string; url: string}) {
  return (
    <article className={styles.card}>
      <h2>{title}</h2>
      <p>{description}</p>
      <Link className={styles.link} to={url}>
        Open sample
      </Link>
    </article>
  );
}

export default function Playground() {
  return (
    <Layout title="Playground" description="Live examples and iterative editing with Fission.">
      <main className={`container ${styles.pageShell}`}>
        <section className={styles.section}>
          <h1 className={styles.heading}>Playground</h1>
          <p className={styles.lead}>
            Use the generated project as your real playground: edit source, rebuild, verify one behavior change,
            then expand to the target that matters for the milestone.
          </p>
        </section>
        <section className={styles.section}>
          <h2>Starter workflows</h2>
          <div className={styles.grid}>
            <DemoCard
              title="Counter"
              description="Signals, reducers, and rebuild loop in one compact flow."
              url="/docs/tutorials/counter"
            />
            <DemoCard
              title="Todo"
              description="Commands, services, and async persistence behavior."
              url="/docs/tutorials/todo"
            />
            <DemoCard
              title="Responsive layout"
              description="Wide-screen dashboard behavior and responsive composition."
              url="/docs/guide/widgets-and-layout"
            />
          </div>
        </section>
        <section className={styles.section}>
          <h2>How to run iterations</h2>
          <p className={styles.lead}>
            Run the generated app with <code>cargo run</code> for the default local desktop flow. When ready to test
            web or native runtimes, add target scripts and run <code>./platforms/web/run-browser.sh</code>,{' '}
            <code>./platforms/ios/run-sim.sh</code>, or <code>./platforms/android/run-emulator.sh</code>.
          </p>
        </section>
      </main>
    </Layout>
  );
}
