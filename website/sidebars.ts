import type {SidebarsConfig} from '@docusaurus/plugin-content-docs';

const sidebars: SidebarsConfig = {
  docsSidebar: [
    'intro',
    {
      type: 'category',
      label: 'Learn',
      link: {
        type: 'doc',
        id: 'learn/overview',
      },
      collapsed: false,
      items: [
        'learn/quickstart',
        'learn/runtime-model',
        'learn/rendering-pipeline',
        'learn/examples-and-targets',
      ],
    },
    {
      type: 'category',
      label: 'Guides',
      link: {
        type: 'doc',
        id: 'guides/app-structure',
      },
      collapsed: false,
      items: [
        'guides/resources-and-async',
        'guides/input-events-text-and-env',
        'guides/layout-and-widgets',
        'guides/theming-and-i18n',
        'guides/media-animation-portals-and-3d',
        'guides/platform-shells-cli-and-testing',
        'guides/testing-and-diagnostics',
      ],
    },
    {
      type: 'category',
      label: 'Cookbook',
      link: {
        type: 'doc',
        id: 'cookbook/build-a-counter',
      },
      collapsed: false,
      items: [
        'cookbook/run-typed-host-work',
        'cookbook/keep-a-timer-or-service-alive',
        'cookbook/theme-and-locale-toggle',
        'cookbook/modal-text-flow',
        'cookbook/add-platform-targets',
        'cookbook/write-a-live-ui-test',
      ],
    },
    {
      type: 'link',
      label: 'Reference overview',
      href: '/reference/overview/overview',
    },
  ],
};

export default sidebars;
