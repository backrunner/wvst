<script lang="ts">
  import ArrowRight from 'phosphor-svelte/lib/ArrowRight';
  import Browsers from 'phosphor-svelte/lib/Browsers';
  import Cpu from 'phosphor-svelte/lib/Cpu';
  import Gauge from 'phosphor-svelte/lib/Gauge';
  import PlayCircle from 'phosphor-svelte/lib/PlayCircle';
  import PlugsConnected from 'phosphor-svelte/lib/PlugsConnected';
  import ShieldCheck from 'phosphor-svelte/lib/ShieldCheck';
  import TerminalWindow from 'phosphor-svelte/lib/TerminalWindow';
  import Waveform from 'phosphor-svelte/lib/Waveform';
  import SignalConsole from './SignalConsole.svelte';
  import { RootLayout } from 'svedocs/theme';
  import type { SvedocsHomeLayoutProps } from 'svedocs/theme';

  export let page: SvedocsHomeLayoutProps['page'];
  export let pages: SvedocsHomeLayoutProps['pages'] = [];
  export let tree: SvedocsHomeLayoutProps['tree'] = [];
  export let search: SvedocsHomeLayoutProps['search'] = [];
  export let config: SvedocsHomeLayoutProps['config'];
  export let loadSearch: SvedocsHomeLayoutProps['loadSearch'] = undefined;
  export let themeComponents: NonNullable<SvedocsHomeLayoutProps['themeComponents']> = {};

  $: isZh = page.locale === 'zh' || page.routePath === '/zh';
  $: docsHref = isZh ? '/docs/zh' : '/docs';
  $: demoHref = isZh ? '/zh/demo' : '/demo';
  $: copy = isZh ? zh : en;
  $: Root = themeComponents.Root ?? RootLayout;

  const en = {
    label: 'WebAudio meets native VST3',
    title: 'Your browser. Your plugins.',
    titleAccent: 'One audio graph.',
    note: 'Open source · Rust + TypeScript · macOS first',
    setup: 'From source to sound.',
    setupBody: 'Build the bridge, start the local runtime, then open the rack. Bring a VST3 effect and an audio file.',
    setupLink: 'Follow the setup guide',
    intro: 'A Rust bridge brings isolated local VST3 processing into real WebAudio graphs.',
    docs: 'Read docs',
    demo: 'Open demo',
    routeTitle: 'One audio path. Clear ownership.',
    routeBody: 'The browser owns timing and interface. The bridge owns trust. Each plugin runs in an isolated worker.',
    browser: 'Browser',
    browserBody: 'AudioWorklet and shared rings keep the realtime thread non-blocking.',
    bridge: 'Rust bridge',
    bridgeBody: 'Pairing, discovery, routing and metrics stay on loopback.',
    worker: 'VST3 worker',
    workerBody: 'Native processing is isolated so one plugin cannot take down the bridge.',
    builtTitle: 'Designed for the whole signal chain.',
    latency: 'Observable latency',
    latencyBody: 'Track jitter, underflows, overflows and worker restarts instead of guessing.',
    recovery: 'Failure isolation',
    recoveryBody: 'Crash, timeout and quarantine policies are part of the runtime contract.',
    control: 'Real plugin control',
    controlBody: 'Scan classes, mount independent instances, edit parameters and restore state.',
    open: 'Open architecture',
    openBody: 'Rust crates, a typed Web SDK and versioned protocols keep every boundary inspectable.',
    ctaTitle: 'Hear the complete path.',
    ctaBody: 'Run the local bridge, choose an audio file and mount a real effect in the browser rack.'
  };

  const zh = {
    label: 'WebAudio 连接原生 VST3',
    title: '你的浏览器。你的插件。',
    titleAccent: '同一张音频图。',
    note: '开源 · Rust + TypeScript · macOS 优先',
    setup: '从源码，到声音。',
    setupBody: '构建 Bridge，启动本地 runtime，再打开机架。准备一个 VST3 效果器和一段音频即可开始。',
    setupLink: '查看安装指南',
    intro: '通过 Rust bridge，把隔离的本机 VST3 处理接入真实 WebAudio graph。',
    docs: '阅读文档',
    demo: '打开 Demo',
    routeTitle: '一条音频路径，清晰的职责边界。',
    routeBody: '浏览器负责时钟与界面，Bridge 负责信任边界，每个插件都在隔离 worker 中运行。',
    browser: '浏览器',
    browserBody: 'AudioWorklet 与共享 ring buffer 让实时线程始终保持非阻塞。',
    bridge: 'Rust Bridge',
    bridgeBody: '配对、发现、路由与指标都限制在 loopback。',
    worker: 'VST3 Worker',
    workerBody: '原生处理独立隔离，单个插件崩溃不会拖垮 Bridge。',
    builtTitle: '为完整的音频链路而设计。',
    latency: '延迟可观测',
    latencyBody: '直接查看 jitter、underflow、overflow 和 worker restart。',
    recovery: '故障隔离',
    recoveryBody: '崩溃、超时和 quarantine 策略是 runtime contract 的一部分。',
    control: '真实插件控制',
    controlBody: '扫描 class、挂载独立实例、编辑参数并恢复 state。',
    open: '开放架构',
    openBody: 'Rust crates、类型化 Web SDK 与版本化协议让边界始终可检查。',
    ctaTitle: '听见完整链路。',
    ctaBody: '运行本地 Bridge，选择音频文件，在浏览器机架中挂载真实效果器。'
  };

  const setupCommand = 'cargo build -p wvst-bridge-server -p wvst-host-worker\nWVST_HOST_WORKER=target/debug/wvst-host-worker \\\n  target/debug/wvst-bridge-server serve';
</script>

<svelte:component this={Root} {config} {page} {pages} {tree} {search} {loadSearch} {themeComponents}>
  <main id="content" class="wvst-home">
    <section class="wvst-hero">
      <div class="wvst-hero-copy">
        <p class="wvst-eyebrow"><Waveform size={18} weight="bold" />{copy.label}</p>
        <h1>{copy.title}<em>{copy.titleAccent}</em></h1>
        <p class="wvst-hero-intro">{copy.intro}</p>
        <div class="wvst-actions">
          <a class="wvst-action wvst-action-primary" href={docsHref}>{copy.docs}<ArrowRight size={18} weight="bold" /></a>
          <a class="wvst-action" href={demoHref}><PlayCircle size={18} weight="bold" />{copy.demo}</a>
        </div>
        <p class="wvst-hero-note">{copy.note}</p>
      </div>

      <SignalConsole {isZh} />
    </section>

    <div class="wvst-spec-strip"><span>01 / WEB AUDIO ↔ NATIVE AUDIO</span><span>VST3 EFFECTS + INSTRUMENTS</span><span>ASYNC BY DESIGN</span></div>

    <section class="wvst-route-section">
      <p class="wvst-section-index">01 / {isZh ? '架构' : 'THE ARCHITECTURE'}</p>
      <div class="wvst-section-copy">
        <h2>{copy.routeTitle}</h2>
        <p>{copy.routeBody}</p>
      </div>
      <div class="wvst-route" aria-label={copy.routeTitle}>
        <article>
          <Browsers size={28} weight="duotone" />
          <strong>{copy.browser}</strong>
          <p>{copy.browserBody}</p>
        </article>
        <ArrowRight class="wvst-route-arrow" size={24} aria-hidden="true" />
        <article>
          <PlugsConnected size={28} weight="duotone" />
          <strong>{copy.bridge}</strong>
          <p>{copy.bridgeBody}</p>
        </article>
        <ArrowRight class="wvst-route-arrow" size={24} aria-hidden="true" />
        <article>
          <Cpu size={28} weight="duotone" />
          <strong>{copy.worker}</strong>
          <p>{copy.workerBody}</p>
        </article>
      </div>
    </section>

    <section class="wvst-capabilities">
      <p class="wvst-section-index">02 / {isZh ? '运行时' : 'THE RUNTIME'}</p>
      <h2>{copy.builtTitle}</h2>
      <div class="wvst-capability-grid">
        <article class="wvst-capability-featured">
          <Gauge size={34} weight="duotone" />
          <strong>{copy.latency}</strong>
          <p>{copy.latencyBody}</p>
          <div class="wvst-metric-labels"><span>LATENCY / p50 · p95 · p99</span><span>JITTER / UNDERFLOW / OVERFLOW</span></div>
        </article>
        <article>
          <ShieldCheck size={30} weight="duotone" />
          <strong>{copy.recovery}</strong>
          <p>{copy.recoveryBody}</p>
        </article>
        <article>
          <TerminalWindow size={30} weight="duotone" />
          <strong>{copy.control}</strong>
          <p>{copy.controlBody}</p>
        </article>
        <article class="wvst-capability-wide">
          <Cpu size={30} weight="duotone" />
          <strong>{copy.open}</strong>
          <p>{copy.openBody}</p>
        </article>
      </div>
    </section>

    <section class="wvst-setup">
      <div>
        <p class="wvst-section-index">03 / {isZh ? '开始使用' : 'GET CONNECTED'}</p>
        <h2>{copy.setup}</h2><p>{copy.setupBody}</p>
        <a class="wvst-text-link" href={`${docsHref}/getting-started`}>{copy.setupLink}<ArrowRight size={18} /></a>
      </div>
      <div class="wvst-terminal"><div><span>LOCAL BRIDGE</span><span>TERMINAL</span></div><pre><code>{setupCommand}</code></pre><p>{isZh ? '另开终端运行' : 'In a second terminal'} <code>npm run docs:dev</code></p></div>
    </section>

    <section class="wvst-final-cta">
      <div>
        <h2>{copy.ctaTitle}</h2>
        <p>{copy.ctaBody}</p>
        <a class="wvst-action" href={demoHref}><PlayCircle size={18} weight="bold" />{copy.demo}</a>
      </div>
    </section>
  </main>
</svelte:component>
