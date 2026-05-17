import Link from '@docusaurus/Link';
import Layout from '@theme/Layout';
import {playgroundFlows, repoExamples} from '../data/siteContent';
import styles from './experience.module.css';

const recommendedExamples = repoExamples.filter((example) =>
  ['counter', 'widget-gallery', 'text-lab', 'web-smoke', 'mobile-smoke'].includes(example.slug),
);

function repoHref(path: string) {
  return `https://github.com/worka-ai/fission/tree/main/${path}`;
}

export default function Playground() {
  return (
    <Layout title='Playground' description='Practical local loops for exploring Fission against the real repo.'>
      <main className={`container ${styles.pageShell}`}>
        <section className={styles.section}>
          <h1 className={styles.heading}>Playground</h1>
          <p className={styles.lead}>
            Treat the checked-in examples and generated hosts as your playground. The point is not a toy demo; the
            point is a fast loop tied to the same runtime and target paths you will use in product code.
          </p>
        </section>
        <section className={styles.section}>
          <p className={styles.eyebrow}>Recommended loops</p>
          <div className={styles.grid}>
            {playgroundFlows.map((flow) => (
              <article className={styles.card} key={flow.title}>
                <h2>{flow.title}</h2>
                <p>{flow.summary}</p>
                <div className={styles.commandStack}>
                  {flow.commands.map((command) => (
                    <code className={styles.commandBlock} key={command}>
                      {command}
                    </code>
                  ))}
                </div>
                {flow.followUp ? <p className={styles.note}>{flow.followUp}</p> : null}
              </article>
            ))}
          </div>
        </section>
        <section className={styles.section}>
          <p className={styles.eyebrow}>Use these repo examples</p>
          <div className={styles.grid}>
            {recommendedExamples.map((example) => (
              <article className={styles.card} key={example.slug}>
                <div className={styles.metaRow}>
                  <span className={styles.pill}>{example.crate}</span>
                  <code>{example.repoPath}</code>
                </div>
                <h2>{example.title}</h2>
                <p>{example.summary}</p>
                <div className={styles.linkRow}>
                  <Link className={styles.link} to={repoHref(example.repoPath)}>
                    Repo path
                  </Link>
                  {example.docsHref ? (
                    <Link className={styles.link} to={example.docsHref}>
                      Docs
                    </Link>
                  ) : null}
                </div>
              </article>
            ))}
          </div>
        </section>
      </main>
    </Layout>
  );
}
