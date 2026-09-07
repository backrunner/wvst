<script lang="ts">
  import Copy from 'phosphor-svelte/lib/Copy';
  import DownloadSimple from 'phosphor-svelte/lib/DownloadSimple';
  export let locale: 'en' | 'zh' = 'en';
  let copied = false;
  let copyFailed = false;
  const command = 'cargo build --release -p wvst-bridge-server -p wvst-host-worker\nWVST_TOKEN=local-dev-token WVST_HOST_WORKER=target/release/wvst-host-worker target/release/wvst-bridge-server serve';
  $: zh = locale === 'zh';
  async function copyCommand() {
    try { await navigator.clipboard.writeText(command); copied = true; copyFailed = false; }
    catch { copyFailed = true; }
  }
</script>

<div class="bridge-install">
  <div class="bridge-install-step">
    <span class="install-number">A</span>
    <div><h3>{zh ? '获取本地 Bridge' : 'Get the local Bridge'}</h3>
      <p>{zh ? '目前请从源码构建；后续安装包会发布在 Releases。' : 'Build from source for now; future packages will appear in Releases.'}</p>
      <a class="studio-button small" href="https://github.com/backrunner/wvst/releases" target="_blank" rel="noreferrer"><DownloadSimple size={15} />{zh ? '查看 Releases' : 'View releases'} ↗</a>
    </div>
  </div>
  <div class="bridge-install-step">
    <span class="install-number">B</span>
    <div><h3>{zh ? '启动，然后连接' : 'Start it. Then connect.'}</h3>
      <p>{zh ? '保持 Bridge 在本机运行，在高级连接设置输入启动时的 token，然后连接并查看已安装的 VST3。' : 'Keep the Bridge running here, enter its token in advanced settings, then connect to discover installed VST3 effects.'}</p>
      <a class="studio-inline-link" href={zh ? '/docs/zh/getting-started' : '/docs/getting-started'}>{zh ? '完整安装指南' : 'Full setup guide'} ↗</a>
    </div>
  </div>
</div>
<details class="bridge-source-build">
  <summary>{zh ? '从源码构建' : 'Build from source'}</summary>
  <p>{zh ? '安装 Rust 后，在 WVST 仓库根目录运行：' : 'With Rust installed, run from the WVST repository root:'}</p>
  <div class="bridge-command"><pre><code>{command}</code></pre><button class="studio-button small" type="button" onclick={copyCommand}><Copy size={14} />{copied ? (zh ? '已复制' : 'Copied') : (zh ? '复制' : 'Copy')}</button></div>
  {#if copyFailed}<p role="status">{zh ? '无法访问剪贴板，请手动选择并复制命令。' : 'Clipboard unavailable. Select and copy the commands manually.'}</p>{/if}
</details>
