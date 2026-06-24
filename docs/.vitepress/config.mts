import { defineConfig } from 'vitepress'

export default defineConfig({
  title: 'Tasken',
  description: 'Universal task execution framework with scheduling, workflow orchestration, DAG support, and plugin system.',
  srcDir: '.',
  outDir: '.vitepress/dist',
  cleanUrls: true,
  ignoreDeadLinks: true,
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
