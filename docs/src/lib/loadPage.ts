import { error, redirect } from '@sveltejs/kit';
import config from 'virtual:svedocs/config';
import pageLoaders from 'virtual:svedocs/page-loaders';
import pages from 'virtual:svedocs/page-index';
import tree from 'virtual:svedocs/tree';
import type { SvedocsPage } from 'svedocs/core';
import { resolveSvedocsPageRoute } from 'svedocs/routes';

export async function loadSvedocsRoute(path = '') {
  const routePath = normalizeRoutePath(path);
  const resolution = resolveSvedocsPageRoute(routePath, pages, config);
  if (resolution.status === 'redirect') redirect(307, resolution.location);
  if (resolution.status === 'missing') error(404, `No svedocs page found for ${routePath}`);
  const page = await loadFullPage(resolution.page);
  return {
    page,
    pages: pages.map((candidate) => candidate.id === page.id ? page : candidate),
    search: [],
    tree,
    config
  };
}

async function loadFullPage(page: SvedocsPage): Promise<SvedocsPage> {
  const loaded = await pageLoaders[page.id]?.();
  return loaded?.default ?? page;
}

function normalizeRoutePath(path: string): string {
  const clean = path.replace(/^\/+|\/+$/g, '');
  return clean ? `/${clean}` : '/';
}
