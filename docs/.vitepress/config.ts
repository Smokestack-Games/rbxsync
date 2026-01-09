import { defineConfig } from 'vitepress'

export default defineConfig({
  title: 'RbxSync',
  description: 'Full property sync for Roblox Studio. Two-way sync, AI integration, and version control.',

  head: [
    ['link', { rel: 'icon', href: '/logo.png' }],
    ['meta', { name: 'theme-color', content: '#c23c40' }],
    ['meta', { property: 'og:type', content: 'website' }],
    ['meta', { property: 'og:title', content: 'RbxSync Docs' }],
    ['meta', { property: 'og:description', content: 'Full property sync for Roblox Studio' }],
  ],

  themeConfig: {
    logo: '/logo.png',
    siteTitle: 'RbxSync Docs',

    nav: [
      { text: 'Home', link: 'https://rbxsync.dev' },
      { text: 'Guide', link: '/getting-started/' },
      { text: 'CLI', link: '/cli/' },
      { text: 'Plugin', link: '/plugin/' },
      { text: 'VS Code', link: '/vscode/' },
      { text: 'File Formats', link: '/file-formats/' },
      { text: 'MCP', link: '/mcp/' },
    ],

    sidebar: {
      '/getting-started/': [
        {
          text: 'Getting Started',
          items: [
            { text: 'Introduction', link: '/getting-started/' },
            { text: 'Installation', link: '/getting-started/installation' },
            { text: 'Quick Start', link: '/getting-started/quick-start' },
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
          ]
        }
      ],
    },

    socialLinks: [
      { icon: 'github', link: 'https://github.com/devmarissa/rbxsync' },
      { icon: 'discord', link: 'https://discord.gg/rbxsync' },
    ],

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
      message: 'Released under the MIT License.',
      copyright: 'Copyright 2024 RbxSync',
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
