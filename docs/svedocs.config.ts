import { defineConfig } from 'svedocs/config';

export default defineConfig({
  site: {
    name: 'WVST',
    url: 'https://wvst-docs.pages.dev',
    title: 'WVST Docs',
    description: 'Run isolated local VST3 plugins inside real WebAudio graphs through a Rust bridge.'
  },
  content: {
    root: 'content',
    docs: 'content/docs',
    pages: 'content/pages'
  },
  theme: {
    readingStyle: 'plain',
    defaultMode: 'system',
    palette: {
      accent: '#985936',
      neutral: 'zinc'
    },
    fonts: {
      sans: '"Avenir Next", "IBM Plex Sans", "Helvetica Neue", sans-serif',
      mono: '"SFMono-Regular", "IBM Plex Mono", "Courier New", monospace',
      display: '"Iowan Old Style", "Baskerville", "Georgia", serif'
    },
    radius: '3px',
    codeTheme: {
      light: 'light-plus',
      dark: 'dark-plus'
    },
    brand: {
      label: 'WVST',
      href: '/',
      logo: '/brand/wvst-mark.svg'
    },
    nav: [
      { label: 'Docs', href: '/docs' },
      { label: 'Demo', href: '/demo' },
      { label: 'GitHub', href: 'https://github.com/backrunner/wvst', external: true }
    ],
    social: [],
    footer: {
      text: 'WVST is an open source Rust and WebAudio bridge for local VST3 processing.',
      links: [
        { label: 'GitHub', href: 'https://github.com/backrunner/wvst', external: true }
      ]
    },
    home: {
      kicker: 'WebAudio meets native VST3',
      primaryAction: { label: 'Read docs', href: '/docs' },
      secondaryAction: { label: 'Open demo', href: '/demo' },
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
      { code: 'en', label: 'English', hreflang: 'en', dir: 'ltr' },
      { code: 'zh', label: '中文', hreflang: 'zh-CN', dir: 'ltr' }
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
