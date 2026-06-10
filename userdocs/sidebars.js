/** @type {import('@docusaurus/plugin-content-docs').SidebarsConfig} */
const sidebars = {
  tutorialSidebar: [
    'intro',
    {
      type: 'category',
      label: 'Overview',
      items: [
        'overview/product-overview',
        'overview/feature-overview',
        'overview/architecture-overview',
      ],
    },
    {
      type: 'category',
      label: 'Getting Started',
      items: [
        'getting-started/installation',
        'getting-started/configuration-basics',
        'getting-started/login',
        'getting-started/first-task-walkthrough',
      ],
    },
    {
      type: 'category',
      label: 'Core Workflow',
      items: [
        'core/create-agent',
        'core/agent-nodes',
        'core/workdir-worktree-strategy',
        'core/run-and-interact',
        'core/view-output',
        'core/review-and-apply-changes',
        'core/session-lifecycle',
      ],
    },
    {
      type: 'category',
      label: 'Prompting',
      items: ['prompting/task-instruction-patterns'],
    },
    {
      type: 'category',
      label: 'Deployment',
      items: [
        'deployment/overview-and-topology',
        'deployment/production-checklist',
        'deployment/remote-node-transport',
        'deployment/vercel-static-userdocs',
      ],
    },
    {
      type: 'category',
      label: 'Advanced Usage',
      items: [
        'advanced/team-workbench',
        'advanced/openapi-and-automation',
        'advanced/connection-status-and-recovery',
      ],
    },
    {
      type: 'category',
      label: 'Operations',
      items: [
        'operations/daily-operations-checklist',
        'operations/security-and-path-safety',
        'operations/notifications',
        'operations/troubleshooting',
        'operations/api-error-reference',
        'operations/faq',
      ],
    },
  ],
};

module.exports = sidebars;
