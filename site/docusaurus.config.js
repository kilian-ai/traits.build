// @ts-check

/** @type {import('@docusaurus/types').Config} */
const config = {
  title: 'traits.build',
  tagline: 'Composable function kernel in pure Rust',
  favicon: 'img/favicon.ico',

  url: 'https://kilian-ai.github.io',
  baseUrl: '/traits.build/',

  organizationName: 'kilian-ai',
  projectName: 'traits.build',
  deploymentBranch: 'gh-pages',
  trailingSlash: false,

  onBrokenLinks: 'throw',
  onBrokenMarkdownLinks: 'warn',

  i18n: {
    defaultLocale: 'en',
    locales: ['en'],
  },

  presets: [
    [
      'classic',
      /** @type {import('@docusaurus/preset-classic').Options} */
      ({
        docs: {
          sidebarPath: './sidebars.js',
          editUrl: 'https://github.com/kilian-ai/traits.build/tree/main/site/',
        },
        blog: false,
        theme: {
          customCss: './src/css/custom.css',
        },
      }),
    ],
  ],

  themeConfig:
    /** @type {import('@docusaurus/preset-classic').ThemeConfig} */
    ({
      colorMode: {
        defaultMode: 'dark',
        disableSwitch: false,
        respectPrefersColorScheme: true,
      },
      navbar: {
        title: 'traits.build',
        items: [
          {
            type: 'docSidebar',
            sidebarId: 'docsSidebar',
            position: 'left',
            label: 'Docs',
          },
          {
            href: 'https://polygrait-api.fly.dev/docs/api',
            label: 'API Reference',
            position: 'left',
          },
          {
            href: 'https://polygrait-api.fly.dev',
            label: 'Live Demo',
            position: 'right',
          },
          {
            href: 'https://github.com/kilian-ai/traits.build',
            label: 'GitHub',
            position: 'right',
          },
        ],
      },
      footer: {
        style: 'dark',
        links: [
          {
            title: 'Documentation',
            items: [
              { label: 'Getting Started', to: '/docs/getting-started' },
              { label: 'Architecture', to: '/docs/architecture' },
              { label: 'Trait Definition', to: '/docs/trait-definition' },
            ],
          },
          {
            title: 'API',
            items: [
              { label: 'REST API', to: '/docs/rest-api' },
              { label: 'CLI Reference', to: '/docs/cli' },
              { label: 'API Docs (Redoc)', href: 'https://polygrait-api.fly.dev/docs/api' },
            ],
          },
          {
            title: 'Links',
            items: [
              { label: 'GitHub', href: 'https://github.com/kilian-ai/traits.build' },
              { label: 'Live Instance', href: 'https://polygrait-api.fly.dev' },
              { label: 'Health Check', href: 'https://polygrait-api.fly.dev/health' },
            ],
          },
        ],
        copyright: `traits.build — a pure Rust kernel, built with traits.`,
      },
      prism: {
        theme: require('prism-react-renderer').themes.github,
        darkTheme: require('prism-react-renderer').themes.dracula,
        additionalLanguages: ['rust', 'toml', 'bash', 'json'],
      },
    }),
};

module.exports = config;
