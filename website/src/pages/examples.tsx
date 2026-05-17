import Link from '@docusaurus/Link';
import Layout from '@theme/Layout';
import {repoExamples} from '../data/siteContent';
import styles from './experience.module.css';

const starters = repoExamples.filter((example) => example.bucket === 'starter' || example.bucket === 'surface');
const productExamples = repoExamples.filter((example) => example.bucket === 'product');
const targetExamples = repoExamples.filter((example) => example.bucket === 'target');

function repoHref(path: string) {
  return `https://github.com/worka-ai/fission/tree/main/${path}`;
}

function ExampleCard({
  title,
  crate,
  repoPath,
  summary,
  features,
  commands,
  docsHref,
  referenceHref,
  testPath,
}: (typeof repoExamples)[number]) {
  return (
    <article className={styles.card}>
      <div className={styles.metaRow}>
        <span className={styles.pill}>{crate}</span>
        <code>{repoPath}</code>
      </div>
      <h2>{title}</h2>
      <p>{summary}</p>
      <ul className={styles.list}>
        {features.map((feature) => (
          <li key={feature}>{feature}</li>
        ))}
      </ul>
      <div className={styles.commandStack}>
        {commands.map((command) => (
          <code className={styles.commandBlock} key={command}>
            {command}
          </code>
        ))}
      </div>
      <div className={styles.linkRow}>
        <Link className={styles.link} to={repoHref(repoPath)}>
          Repo path
        </Link>
        {docsHref ? (
          <Link className={styles.link} to={docsHref}>
            Docs
          </Link>
        ) : null}
        {referenceHref ? (
          <Link className={styles.link} to={referenceHref}>
            Reference
          </Link>
        ) : null}
        {testPath ? (
          <Link className={styles.link} to={repoHref(testPath)}>
            Tests / notes
          </Link>
        ) : null}
      </div>
    </article>
  );
}

function ExampleSection({title, lead, entries}: {title: string; lead: string; entries: typeof repoExamples}) {
  return (
    <section className={styles.section}>
      <p className={styles.eyebrow}>{title}</p>
      <p className={styles.lead}>{lead}</p>
      <div className={styles.grid}>
        {entries.map((entry) => (
          <ExampleCard key={entry.slug} {...entry} />
        ))}
      </div>
    </section>
  );
}

export default function Examples() {
  return (
    <Layout title='Examples' description='Checked-in Fission examples backed by real repo content and tests.'>
      <main className={`container ${styles.pageShell}`}>
        <section className={styles.section}>
          <h1 className={styles.heading}>Examples</h1>
          <p className={styles.lead}>
            Every entry on this page maps to a real crate under <code>examples/</code>. Use it to pick the
            smallest proof for the subsystem you want to understand.
          </p>
        </section>
        <ExampleSection
          title='Start here'
          lead='Use these when you are learning the app loop, the widget surface, or the text/input model.'
          entries={starters}
        />
        <ExampleSection
          title='Product-like samples'
          lead='Use these when you need proof that the framework can host layered, stateful application UI.'
          entries={productExamples}
        />
        <ExampleSection
          title='Target smoke paths'
          lead='Use these when you need concrete browser or mobile host flows instead of high-level target claims.'
          entries={targetExamples}
        />
      </main>
    </Layout>
  );
}
