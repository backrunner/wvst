import { defineConfig } from 'svedocs/config';

export default defineConfig({
  site: {
    name: 'WVST',
    title: 'WVST Docs',
    description: 'A Rust and WebAudio bridge for running local VST effects from the browser.'
  },
  content: {
    root: 'content',
    docs: 'content/docs',
    pages: 'content/pages'
  },
  theme: {
    defaultMode: 'system',
    palette: {
      accent: '#9b5a2e',
      neutral: 'zinc'
    },
    fonts: {
      sans: '"Avenir Next", "IBM Plex Sans", "Helvetica Neue", sans-serif',
      mono: '"JetBrains Mono", "SFMono-Regular", monospace',
      display: '"Iowan Old Style", "Georgia", serif'
    },
    radius: '3px',
    codeTheme: {
      light: 'light-plus',
      dark: 'dark-plus'
    },
    brand: {
      label: 'WVST',
      href: '/',
      logo: '/favicon.svg'
    },
    nav: [
      { label: 'Docs', href: '/docs' },
      { label: 'Live Demo', href: '/docs/live-demo' },
      { label: '中文', href: '/docs/zh' }
    ],
    social: [],
    footer: {
      text: 'WVST is an experimental local VST bridge for the web.',
      links: []
    },
    home: {
      kicker: 'Local VST effects in WebAudio',
      primaryAction: { label: 'Read docs', href: '/docs' },
      secondaryAction: { label: 'Open demo', href: '/docs/live-demo' },
      visual: { type: 'pixel', alt: '' }
    }
  },
  search: {
    enabled: true,
    provider: 'local',
    scope: 'current'
  },
  ai: false,
  i18n: {
    defaultLocale: 'en',
    locales: [
      { code: 'en', label: 'English' },
      { code: 'zh', label: '中文' }
    ]
  },
  seo: {
    defaultAuthor: 'WVST team',
    ogImage: {
      template: 'default',
      format: 'svg',
      outDir: 'static/og',
      renderer: 'svg'
    }
  }
});
