import {themes as prismThemes} from 'prism-react-renderer';
import type {Config} from '@docusaurus/types';
import type * as Preset from '@docusaurus/preset-classic';

const config: Config = {
  title: 'Fission',
  tagline: 'Build desktop and web apps in Rust from one codebase',
  favicon: 'img/favicon.ico',
  future: {
    v4: true,
  },
  url: 'https://fission.dev',
  baseUrl: '/',
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
        routeBasePath: '/reference',
        sidebarPath: './reference-sidebars.ts',
        editUrl: 'https://github.com/worka-ai/fission/edit/main/website/reference',
      },
    ],
  ],
  themeConfig: {
    image: 'img/docusaurus-social-card.jpg',
    colorMode: {
      respectPrefersColorScheme: true,
    },
    navbar: {
      title: 'Fission',
      logo: {
        alt: 'Fission logo',
        src: 'img/logo.svg',
      },
      items: [
        {
          to: '/docs/getting-started/what-is-fission',
          label: 'Docs',
          position: 'left',
          activeBasePath: '/docs',
        },
        {
          to: '/reference/overview/',
          label: 'Reference',
          position: 'left',
          activeBasePath: '/reference',
        },
        {to: '/examples', label: 'Examples', position: 'left'},
        {to: '/playground', label: 'Playground', position: 'left'},
        {to: '/showcase', label: 'Showcase', position: 'left'},
        {to: '/blog', label: 'Blog', position: 'right'},
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
            {
              label: 'Why Fission',
              to: '/docs/getting-started/what-is-fission',
            },
            {
              label: 'First App',
              to: '/docs/getting-started/first-app',
            },
            {
              label: 'Playground',
              to: '/playground',
            },
            {
              label: 'Accessibility and i18n',
              to: '/docs/guide/i18n-and-accessibility',
            },
          ],
        },
        {
          title: 'Reference',
          items: [
            {
              label: 'Core Framework',
              to: '/reference/core/widget-trait',
            },
            {
              label: 'CLI',
              to: '/reference/cli/overview',
            },
            {
              label: 'Widgets',
              to: '/reference/widgets/catalog',
            },
          ],
        },
        {
          title: 'Community',
          items: [
            {
              label: 'Blog',
              to: '/blog',
            },
            {
              label: 'Examples',
              to: '/examples',
            },
            {
              label: 'GitHub',
              href: 'https://github.com/worka-ai/fission',
            },
          ],
        },
      ],
      copyright: `Copyright © ${new Date().getFullYear()} Fission. Built with Docusaurus. Fission is actively being developed; core runtime and docs workflow are in active use while adjacent tooling and ecosystem support continue to improve.`,
      logo: {
        alt: 'Fission',
        src: 'img/logo.svg',
      },
    },
    prism: {
      theme: prismThemes.github,
      darkTheme: prismThemes.dracula,
    },
  } satisfies Preset.ThemeConfig,
};

export default config;
