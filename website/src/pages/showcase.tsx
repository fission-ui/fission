import Link from '@docusaurus/Link';
import Layout from '@theme/Layout';
import {showcaseStories} from '../data/siteContent';
import styles from './experience.module.css';

function repoHref(path: string) {
  return `https://github.com/worka-ai/fission/tree/main/${path}`;
}

export default function Showcase() {
  return (
    <Layout title='Showcase' description='The most product-proof examples and target stories currently checked into the Fission repo.'>
      <main className={`container ${styles.pageShell}`}>
        <section className={styles.section}>
          <h1 className={styles.heading}>Showcase</h1>
          <p className={styles.lead}>
            These are the stories worth pointing at when someone asks whether the framework already covers more than a
            starter counter. Every card on this page maps to real code in this repo.
          </p>
        </section>
        <section className={styles.section}>
          <div className={styles.grid}>
            {showcaseStories.map((story) => (
              <article className={styles.card} key={story.title}>
                <div className={styles.metaRow}>
                  <span className={styles.pill}>repo-backed</span>
                  <code>{story.repoPath}</code>
                </div>
                <h2>{story.title}</h2>
                <p>{story.summary}</p>
                <ul className={styles.list}>
                  {story.proofs.map((proof) => (
                    <li key={proof}>{proof}</li>
                  ))}
                </ul>
                <div className={styles.linkRow}>
                  <Link className={styles.link} to={repoHref(story.repoPath)}>
                    Open repo path
                  </Link>
                  <Link className={styles.link} to={story.href}>
                    Related page
                  </Link>
                </div>
              </article>
            ))}
          </div>
        </section>
      </main>
    </Layout>
  );
}
