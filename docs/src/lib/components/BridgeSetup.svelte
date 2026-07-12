<script lang="ts">
  import ArrowSquareOut from 'phosphor-svelte/lib/ArrowSquareOut';
  import DownloadSimple from 'phosphor-svelte/lib/DownloadSimple';

  export let locale: 'en' | 'zh' = 'en';

  const releasesHref = 'https://github.com/backrunner/wvst/releases';
  const sourceHref = 'https://github.com/backrunner/wvst';
  const copy = {
    en: {
      eyebrow: 'Before you connect',
      title: 'Install the local Bridge',
      body: 'WVST uses a local Bridge to run native VSTs. Keep it running; this demo tries to connect when the page opens.',
      download: 'Download Bridge',
      source: 'View source',
      sourceTitle: 'No release yet? Build it locally.',
      sourceBody: 'From the repository root:',
      build: 'cargo build --release -p wvst-bridge-server -p wvst-host-worker',
      run: 'WVST_HOST_WORKER=target/release/wvst-host-worker target/release/wvst-bridge-server serve',
      note: 'The server listens on ws://127.0.0.1:35876 by default.'
    },
    zh: {
      eyebrow: '连接前先准备',
      title: '安装本地 Bridge',
      body: 'WVST 通过本地 Bridge 运行原生 VST。保持 Bridge 运行，Demo 会在页面打开时自动尝试连接。',
      download: '下载 Bridge',
      source: '查看源码',
      sourceTitle: '还没有 release？从源码构建。',
      sourceBody: '在仓库根目录执行：',
      build: 'cargo build --release -p wvst-bridge-server -p wvst-host-worker',
      run: 'WVST_HOST_WORKER=target/release/wvst-host-worker target/release/wvst-bridge-server serve',
      note: '服务默认监听 ws://127.0.0.1:35876。'
    }
  };

  $: t = copy[locale] ?? copy.en;
</script>

<section class="wvst-bridge-setup" aria-labelledby="wvst-bridge-setup-title">
  <div class="wvst-bridge-setup-main">
    <span class="wvst-bridge-setup-eyebrow">{t.eyebrow}</span>
    <h2 id="wvst-bridge-setup-title">{t.title}</h2>
    <p>{t.body}</p>
    <div class="wvst-bridge-setup-actions">
      <a class="wvst-setup-action wvst-setup-action-primary" href={releasesHref} target="_blank" rel="noreferrer">
        <DownloadSimple size={17} weight="bold" aria-hidden="true" />
        {t.download}
      </a>
      <a class="wvst-setup-action" href={sourceHref} target="_blank" rel="noreferrer">
        <ArrowSquareOut size={16} aria-hidden="true" />
        {t.source}
      </a>
    </div>
  </div>

  <div class="wvst-bridge-setup-source">
    <div class="wvst-bridge-setup-source-heading">
      <strong>{t.sourceTitle}</strong>
      <span>{t.sourceBody}</span>
    </div>
    <code>{t.build}</code>
    <code>{t.run}</code>
    <small>{t.note}</small>
  </div>
</section>
