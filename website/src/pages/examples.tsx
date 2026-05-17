import Link from '@docusaurus/Link';
import Layout from '@theme/Layout';
import {repoExamples, type RepoExample} from '../data/siteContent';
import styles from './experience.module.css';

const exampleMap = new Map(repoExamples.map((example) => [example.slug, example]));

const startHere = ['counter', 'widget-gallery', 'text-lab'].map((slug) => exampleMap.get(slug)!);
const productionExamples = ['inbox', 'terminal', 'editor', 'chart-gallery'].map(
  (slug) => exampleMap.get(slug)!,
);
const surfaceExamples = [
  'animation-gallery',
  'icons-gallery',
  'embed-video',
  'embed-webview',
  'embed-3d',
].map((slug) => exampleMap.get(slug)!);
const targetExamples = ['web-smoke', 'mobile-smoke'].map((slug) => exampleMap.get(slug)!);

const routes = [
  {
    title: 'I want the smallest complete app loop',
    body:
      'Start with Counter. It is small enough to read in one sitting, but it still shows the full shape of a Fission app: app state, typed actions, reducers, a selector-backed view model, a widget tree, and a modal overlay.',
    href: '/docs/learn/runtime-model',
    label: 'Open runtime model',
  },
  {
    title: 'I want to compose real screens from built-in widgets',
    body:
      'Open Widget Gallery next. It gives you a broad tour of layout, inputs, overlays, navigation, data widgets, and feedback widgets without forcing you to assemble a catalog from source files.',
    href: '/docs/guides/layout-and-widgets',
    label: 'Read the layout guide',
  },
  {
    title: 'I care about text input and editing edge cases',
    body:
      'Go straight to Text Lab. It isolates text input, menus, comboboxes, modal focus, and input method editor flows so you can validate the hardest parts before they disappear into a bigger product.',
    href: '/docs/guides/input-events-text-and-env',
    label: 'Read the input guide',
  },
  {
    title: 'I want a product-shaped example, not a teaching toy',
    body:
      'Open Inbox or Fission Editor. Inbox proves responsive navigation, theme and locale switching, and host capabilities. Fission Editor proves jobs, timers, custom rendering, portals, and large live tests.',
    href: '/showcase',
    label: 'See showcase stories',
  },
  {
    title: 'I need to prove a browser or mobile host path',
    body:
      'Use Web Smoke and Mobile Smoke when browser, Android, or iOS host behavior is part of the question. They prove real launch paths around the same shared app model, so you can validate packaging, host setup, and target-specific behavior directly.',
    href: '/docs/learn/examples-and-targets',
    label: 'Read examples and targets',
  },
];

function repoHref(path: string) {
  return `https://github.com/worka-ai/fission/tree/main/${path}`;
}

function ExampleCard({example}: {example: RepoExample}) {
  return (
    <article className={styles.storyCard}>
      <div className={styles.metaRow}>
        <span className={styles.pill}>{example.crate}</span>
        <code>{example.repoPath}</code>
      </div>
      <h2>{example.title}</h2>
      <div className={styles.storyBody}>
        <p>{example.summary}</p>
        <p>
          Open this example when you specifically want to study{' '}
          {example.features.length === 1
            ? example.features[0]
            : `${example.features.slice(0, -1).join(', ')}, and ${example.features.at(-1)}`}
          . The goal is not to copy the crate line for line. The goal is to see how one question is solved in a running app that already matches the real runtime.
        </p>
      </div>
      <ul className={styles.detailList}>
        {example.features.map((feature) => (
          <li key={feature}>{feature}</li>
        ))}
      </ul>
      <div className={styles.commandStack}>
        {example.commands.map((command) => (
          <code className={styles.commandBlock} key={command}>
            {command}
          </code>
        ))}
      </div>
      <div className={styles.linkRow}>
        <Link className={styles.link} to={repoHref(example.repoPath)}>
          Open repo path
        </Link>
        {example.docsHref ? (
          <Link className={styles.link} to={example.docsHref}>
            Related docs
          </Link>
        ) : null}
        {example.testPath ? (
          <Link className={styles.link} to={repoHref(example.testPath)}>
            Tests or notes
          </Link>
        ) : null}
      </div>
    </article>
  );
}

function ExampleSection({
  eyebrow,
  title,
  lead,
  entries,
}: {
  eyebrow: string;
  title: string;
  lead: string;
  entries: RepoExample[];
}) {
  return (
    <section className={styles.section}>
      <p className={styles.eyebrow}>{eyebrow}</p>
      <h2 className={styles.heading}>{title}</h2>
      <p className={styles.lead}>{lead}</p>
      <div className={styles.storyGrid}>
        {entries.map((example) => (
          <ExampleCard key={example.slug} example={example} />
        ))}
      </div>
    </section>
  );
}

export default function Examples() {
  return (
    <Layout title='Examples' description='Repo-backed Fission examples with guidance on what to open first and why.'>
      <main className={`container ${styles.pageShell}`}>
        <section className={styles.section}>
          <h1 className={styles.heading}>Examples</h1>
          <p className={styles.lead}>
            Every example on this page maps to a real crate under <code>examples/</code>. Use them as guided proofs for specific product problems. They are here to answer concrete questions about the framework and its targets, not to replace the docs or force you to reverse-engineer the architecture from application code alone.
          </p>
          <div className={styles.heroPanel}>
            <div className={styles.heroCopy}>
              <p>
                The quickest way to learn Fission is to choose one question, run the example that already proves that part of the system, and then read the matching guide while the behavior is still fresh in your head. That keeps the examples grounded in explanation instead of turning them into a scavenger hunt through source files.
              </p>
              <p>
                Choose the first host that matches the behavior you actually need to prove. Desktop is often the shortest local loop for shared runtime questions such as reducers, widgets, selectors, and layout. Browser and mobile examples are the right first stop when your question is really about browser hosting, Android packaging, iOS simulator behavior, viewport constraints, or another target-specific concern. The constant across all of them is one shared app model.
              </p>
            </div>
            <aside className={styles.heroAside}>
              <p className={styles.smallHeading}>Choose your first example by question</p>
              <div className={styles.stackLinks}>
                {routes.map((route) => (
                  <div key={route.title}>
                    <p>
                      <strong>{route.title}</strong>
                    </p>
                    <p>{route.body}</p>
                    <Link className={styles.link} to={route.href}>
                      {route.label}
                    </Link>
                  </div>
                ))}
              </div>
            </aside>
          </div>
        </section>

        <ExampleSection
          eyebrow='Start here'
          title='These three examples explain the shared app model most clearly'
          lead='Counter explains the update loop, Widget Gallery shows how ordinary screens are composed, and Text Lab isolates the text and focus problems that often become fragile first.'
          entries={startHere}
        />

        <ExampleSection
          eyebrow='Product-shaped examples'
          title='Use these when you need proof that the architecture scales into real application shells'
          lead='These crates matter once you care about multi-panel navigation, longer-lived background work, custom rendering, richer input flows, and the kinds of interactions production teams actually have to maintain.'
          entries={productionExamples}
        />

        <ExampleSection
          eyebrow='Focused surfaces'
          title='Open these when you want one rendering or presentation question in isolation'
          lead='Animation Gallery and Icons Gallery are narrower than the product-shaped apps, but that is exactly why they are useful. They let you inspect motion primitives and visual assets without the rest of an application shell getting in the way.'
          entries={surfaceExamples}
        />

        <ExampleSection
          eyebrow='Targets'
          title='Use these when the host itself is part of the problem you are solving'
          lead='Web Smoke and Mobile Smoke prove that the same shared runtime can be packaged and launched through real browser, Android, and iOS host paths. Open them whenever you need to validate host setup, launch behavior, WebAssembly packaging, emulator or simulator flow, or another target-specific concern.'
          entries={targetExamples}
        />
      </main>
    </Layout>
  );
}
