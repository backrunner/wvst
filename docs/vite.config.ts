import tailwindcss from '@tailwindcss/vite';
import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';
import { svedocs } from 'svedocs/vite';
import svedocsConfig from './svedocs.config';

export default defineConfig({
  server: {
    headers: {
      'Cross-Origin-Embedder-Policy': 'require-corp',
      'Cross-Origin-Opener-Policy': 'same-origin'
    }
  },
  plugins: [
    svedocs({
      config: svedocsConfig,
      components: {
        WVSTRackDemo: '$lib/components/WVSTRackDemo.svelte'
      }
    }),
    tailwindcss(),
    sveltekit()
  ]
});
