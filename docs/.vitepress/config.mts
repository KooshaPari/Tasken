import { defineConfig } from 'vitepress'

export default defineConfig({
  title: 'Tasken',
  description: 'Universal task execution framework with scheduling, workflow orchestration, DAG support, and plugin system.',
  srcDir: '.',
  outDir: '.vitepress/dist',
  cleanUrls: true,
  ignoreDeadLinks: true,
  // Exclude non-public propagated files (boundary/intent/history/worklogs/etc.
  // are consumed by other tools and contain HTML-like tags in YAML frontmatter
  // that VitePress's Vue template compiler cannot parse). Only the public docs
  // surface (root + getting-started) is shipped.
  srcExclude: [
    'boundary/**',
    'intent/**',
    'history/**',
    'worklogs/**',
    'traceability/**',
    'operations/**',
    'stories/**',
    'journeys/**',
  ],
  themeConfig: {
    nav: [
      { text: 'Home', link: '/' },
      { text: 'Getting Started', link: '/getting-started' },
    ],
    sidebar: [
      {
        text: 'Introduction',
        items: [
          { text: 'Overview', link: '/' },
          { text: 'Getting Started', link: '/getting-started' },
        ],
      },
    ],
    socialLinks: [
      { icon: 'github', link: 'https://github.com/KooshaPari/Tasken' },
    ],
  },
})
