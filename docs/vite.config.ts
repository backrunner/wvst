import tailwindcss from '@tailwindcss/vite';
import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';
import { svedocs } from 'svedocs/vite';
import svedocsConfig from './svedocs.config';

const isolationHeaders = {
  'Cross-Origin-Embedder-Policy': 'require-corp',
  'Cross-Origin-Opener-Policy': 'same-origin'
};

export default defineConfig({
  server: { headers: isolationHeaders },
  plugins: [
    {
      name: 'wvst-preview-isolation',
      configurePreviewServer(server) {
        // SvelteKit serves prerendered pages before Vite's static middleware.
        server.middlewares.use((_request, response, next) => {
          for (const [name, value] of Object.entries(isolationHeaders)) {
            response.setHeader(name, value);
          }
          next();
        });
      }
    },
    svedocs({
      config: svedocsConfig,
      layouts: { studio: '$lib/theme/WVSTStudioLayout.svelte' },
      components: {
        WVSTRackDemo: '$lib/components/WVSTRackDemo.svelte',
        BridgeSetup: '$lib/components/BridgeSetup.svelte'
      },
      theme: {
        components: {
          Brand: '$lib/theme/WVSTBrand.svelte',
          Home: '$lib/theme/WVSTHome.svelte'
        }
      }
    }),
    tailwindcss(),
    sveltekit()
  ]
});
