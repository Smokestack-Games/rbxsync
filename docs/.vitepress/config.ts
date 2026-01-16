import { defineConfig } from 'vitepress'

export default defineConfig({
  title: 'RbxSync Docs',
  description: 'Documentation for RbxSync – two-way sync between Roblox Studio and VS Code.',
  cleanUrls: true,

  head: [
    ['link', { rel: 'icon', href: '/logo.png' }],
    ['meta', { name: 'theme-color', content: '#c23c40' }],
    ['meta', { property: 'og:type', content: 'website' }],
    ['meta', { property: 'og:site_name', content: 'RbxSync' }],
    ['meta', { property: 'og:title', content: 'RbxSync Docs' }],
    ['meta', { property: 'og:description', content: 'Documentation for RbxSync – two-way sync between Roblox Studio and VS Code.' }],
    ['meta', { property: 'og:image', content: 'https://docs.rbxsync.dev/og-image.png' }],
    ['meta', { name: 'twitter:card', content: 'summary' }],
    ['meta', { name: 'twitter:title', content: 'RbxSync Docs' }],
    ['meta', { name: 'twitter:description', content: 'Documentation for RbxSync – two-way sync between Roblox Studio and VS Code.' }],
  ],

  themeConfig: {
    logo: { src: '/logo.png', link: 'https://rbxsync.dev' },
    siteTitle: 'RbxSync',

    nav: [
      { text: 'Guide', link: '/getting-started/' },
      {
        text: 'Reference',
        items: [
          { text: 'CLI', link: '/cli/' },
          { text: 'Plugin', link: '/plugin/' },
          { text: 'VS Code', link: '/vscode/' },
          { text: 'File Formats', link: '/file-formats/' },
          { text: 'MCP', link: '/mcp/' },
        ]
      },
      { text: 'FAQ', link: '/faq' },
    ],

    sidebar: {
      '/getting-started/': [
        {
          text: 'Getting Started',
          items: [
            { text: 'Introduction', link: '/getting-started/' },
            { text: 'Installation', link: '/getting-started/installation' },
            { text: 'Quick Start', link: '/getting-started/quick-start' },
            { text: 'Configuration', link: '/getting-started/configuration' },
          ]
        }
      ],
      '/cli/': [
        {
          text: 'CLI Reference',
          items: [
            { text: 'Overview', link: '/cli/' },
            { text: 'Commands', link: '/cli/commands' },
            { text: 'Build', link: '/cli/build' },
          ]
        }
      ],
      '/plugin/': [
        {
          text: 'Studio Plugin',
          items: [
            { text: 'Overview', link: '/plugin/' },
            { text: 'Installation', link: '/plugin/installation' },
            { text: 'Usage', link: '/plugin/usage' },
          ]
        }
      ],
      '/vscode/': [
        {
          text: 'VS Code Extension',
          items: [
            { text: 'Overview', link: '/vscode/' },
            { text: 'Commands', link: '/vscode/commands' },
            { text: 'E2E Testing', link: '/vscode/e2e-testing' },
          ]
        }
      ],
      '/file-formats/': [
        {
          text: 'File Formats',
          items: [
            { text: 'Overview', link: '/file-formats/' },
            { text: '.luau Scripts', link: '/file-formats/luau' },
            { text: '.rbxjson Format', link: '/file-formats/rbxjson' },
            { text: 'Property Types', link: '/file-formats/property-types' },
          ]
        }
      ],
      '/mcp/': [
        {
          text: 'MCP Integration',
          items: [
            { text: 'Overview', link: '/mcp/' },
            { text: 'Setup', link: '/mcp/setup' },
            { text: 'Tools', link: '/mcp/tools' },
            { text: 'AI Testing Workflow', link: '/ai-testing' },
          ]
        }
      ],
    },

    // Using custom nav buttons instead of default socialLinks

    search: {
      provider: 'local',
      options: {
        detailedView: true,
        miniSearch: {
          searchOptions: {
            fuzzy: 0.2,
            prefix: true,
            boost: { title: 4, text: 2 },
          },
        },
      },
    },

    editLink: {
      pattern: 'https://github.com/devmarissa/rbxsync/edit/master/docs/:path',
      text: 'Edit this page on GitHub',
    },

    footer: {
      message: '',
      copyright: '© 2026 <a href="https://smokestackgames.com/" target="_blank" rel="noopener noreferrer">Smokestack Games</a>',
    },
  },

  markdown: {
    theme: {
      light: 'github-light',
      dark: 'github-dark',
    },
    lineNumbers: true,
  },
})
