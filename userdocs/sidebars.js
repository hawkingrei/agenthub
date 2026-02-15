/** @type {import('@docusaurus/plugin-content-docs').SidebarsConfig} */
const sidebars = {
  tutorialSidebar: [
    'intro',
    {
      type: 'category',
      label: 'Getting Started',
      items: ['getting-started/installation', 'getting-started/login'],
    },
    {
      type: 'category',
      label: 'Core Workflow',
      items: [
        'core/create-agent',
        'core/run-and-interact',
        'core/view-output',
      ],
    },
    {
      type: 'category',
      label: 'Operations',
      items: ['operations/notifications', 'operations/troubleshooting'],
    },
  ],
};

module.exports = sidebars;
