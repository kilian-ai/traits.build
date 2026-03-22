/** @type {import('@docusaurus/plugin-content-docs').SidebarsConfig} */
const sidebars = {
  docsSidebar: [
    'intro',
    'getting-started',
    {
      type: 'category',
      label: 'Core Concepts',
      items: [
        'architecture',
        'trait-definition',
        'interfaces',
        'type-system',
      ],
    },
    {
      type: 'category',
      label: 'Reference',
      items: [
        'rest-api',
        'cli',
      ],
    },
    {
      type: 'category',
      label: 'Guides',
      items: [
        'creating-traits',
        'deployment',
      ],
    },
  ],
};

module.exports = sidebars;
