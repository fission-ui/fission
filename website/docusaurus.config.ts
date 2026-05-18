import {themes as prismThemes} from 'prism-react-renderer';
import type {Config} from '@docusaurus/types';
import type * as Preset from '@docusaurus/preset-classic';

const siteUrl = process.env.DOCS_SITE_URL ?? 'https://fission.dev';
const siteBaseUrl = process.env.DOCS_BASE_URL ?? '/';

const config: Config = {
  title: 'Fission',
  tagline: 'Deterministic Rust user interface with a shared runtime for desktop, web, iOS, and Android.',
  favicon: 'img/fission_logo.png',
  future: {
    v4: true,
  },
  url: siteUrl,
  baseUrl: siteBaseUrl,
  organizationName: 'worka-ai',
  projectName: 'fission',
  onBrokenLinks: 'throw',
  i18n: {
    defaultLocale: 'en',
    locales: ['en'],
  },
  presets: [
    [
      'classic',
      {
        docs: {
          sidebarPath: './sidebars.ts',
          editUrl: 'https://github.com/worka-ai/fission/edit/main/website',
        },
        blog: {
          showReadingTime: true,
          feedOptions: {
            type: ['rss', 'atom'],
            xslt: true,
          },
          editUrl: 'https://github.com/worka-ai/fission/edit/main/website',
          onInlineTags: 'warn',
          onInlineAuthors: 'warn',
          onUntruncatedBlogPosts: 'warn',
        },
        theme: {
          customCss: './src/css/custom.css',
        },
      } satisfies Preset.Options,
    ],
  ],
  plugins: [
    [
      '@docusaurus/plugin-content-docs',
      {
        id: 'reference',
        path: 'reference',
        routeBasePath: 'reference',
        sidebarPath: './reference-sidebars.ts',
        editUrl: 'https://github.com/worka-ai/fission/edit/main/website/reference',
      },
    ],
  ],
  themeConfig: {
    image: 'img/fission_logo.png',
    colorMode: {
      respectPrefersColorScheme: true,
    },
    navbar: {
      title: 'Fission',
      logo: {
        alt: 'Fission logo',
        src: 'img/fission_logo.png',
      },
      items: [
        {
          to: '/docs/learn/overview',
          label: 'Learn',
          position: 'left',
          activeBasePath: '/docs/learn',
        },
        {
          to: '/docs/guides/app-structure',
          label: 'Guides',
          position: 'left',
          activeBasePath: '/docs/guides',
        },
        {
          to: '/docs/charts/overview',
          label: 'Charts',
          position: 'left',
          activeBasePath: '/docs/charts',
        },
        {
          to: '/docs/cookbook/build-a-counter',
          label: 'Cookbook',
          position: 'left',
          activeBasePath: '/docs/cookbook',
        },
        {
          to: '/reference/overview/overview',
          label: 'Reference',
          position: 'left',
          activeBasePath: '/reference',
        },
        {to: '/examples', label: 'Examples', position: 'left'},
        {
          href: 'https://github.com/worka-ai/fission',
          label: 'GitHub',
          position: 'right',
        },
      ],
    },
    footer: {
      style: 'dark',
      links: [
        {
          title: 'Learn',
          items: [
            {label: 'Overview', to: '/docs/learn/overview'},
            {label: 'Quickstart', to: '/docs/learn/quickstart'},
            {label: 'Runtime model', to: '/docs/learn/runtime-model'},
          ],
        },
        {
          title: 'Guides',
          items: [
            {label: 'App structure', to: '/docs/guides/app-structure'},
            {label: 'Resources and async', to: '/docs/guides/resources-and-async'},
            {label: 'Testing and diagnostics', to: '/docs/guides/testing-and-diagnostics'},
          ],
        },
        {
          title: 'Charts',
          items: [
            {label: 'Overview', to: '/docs/charts/overview'},
            {label: 'Catalog', to: '/docs/charts/catalog'},
            {label: 'Data and interaction', to: '/docs/charts/data-and-interaction'},
            {label: '3D and GL', to: '/docs/charts/three-dimensional-and-gl'},
          ],
        },
        {
          title: 'Cookbook',
          items: [
            {label: 'Build a counter', to: '/docs/cookbook/build-a-counter'},
            {label: 'Add platform targets', to: '/docs/cookbook/add-platform-targets'},
            {label: 'Write a live interface test', to: '/docs/cookbook/write-a-live-ui-test'},
          ],
        },
        {
          title: 'Explore',
          items: [
            {label: 'Reference', to: '/reference/overview/overview'},
            {label: 'Examples', to: '/examples'},
            {label: 'Playground', to: '/playground'},
            {label: 'Showcase', to: '/showcase'},
            {label: 'GitHub', href: 'https://github.com/worka-ai/fission'},
          ],
        },
      ],
      copyright: `Copyright © ${new Date().getFullYear()} Fission. The Fission framework is ready to use today but some areas are actively under development. Widget APIs are expected to remain stable but some runtime or shell APIs may get breaking changes before we get to a 1.0.0 release`,
      logo: {
        alt: 'Fission',
        src: 'img/fission_logo.png',
      },
    },
    prism: {
      theme: prismThemes.github,
      darkTheme: prismThemes.dracula,
    },
  } satisfies Preset.ThemeConfig,
};

export default config;
