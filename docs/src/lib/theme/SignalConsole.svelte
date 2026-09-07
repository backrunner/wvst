<script lang="ts">
  export let isZh = false;
  let selected = 0;
  $: stages = isZh ? [
    { name: 'WebAudio', tag: '浏览器', title: '让音频时钟持续前进。', body: 'AudioWorklet 通过共享环形缓冲交换音频，无需等待本地插件返回。', detail: 'AudioWorklet + SharedArrayBuffer' },
    { name: 'Rust Bridge', tag: '本地桥接', title: '每一条连接都有边界。', body: '本地 Bridge 管理授权、插件发现与音频路由，并记录延迟和丢帧指标。', detail: 'Loopback · 二进制音频协议' },
    { name: 'VST3', tag: '隔离进程', title: '原生插件，独立运行。', body: 'Host worker 承载插件处理。独立实例保留各自的参数与状态。', detail: '效果器 · 音源 · MIDI' }
  ] : [
    { name: 'WebAudio', tag: 'BROWSER', title: 'Keep the audio clock moving.', body: 'The AudioWorklet exchanges audio through shared rings without waiting for native processing.', detail: 'AudioWorklet + SharedArrayBuffer' },
    { name: 'Rust Bridge', tag: 'LOCAL BRIDGE', title: 'Give every connection a boundary.', body: 'The local bridge owns authorization, discovery and routing, with latency and drop metrics.', detail: 'Loopback · Binary audio protocol' },
    { name: 'VST3', tag: 'ISOLATED PROCESS', title: 'Native plugins. Independent lives.', body: 'Host workers run native processing. Each instance keeps its own parameters and state.', detail: 'Effects · Instruments · MIDI' }
  ];
  const bars = Array.from({ length: 64 }, (_, i) => 12 + Math.abs(Math.sin(i * .42) * Math.cos(i * .13)) * 78);
</script>

<div class="signal-console">
  <div class="console-top"><span class="console-brand">WVST <span>/ SIGNAL PATH</span></span><span class="console-id">001—003</span></div>
  <div class="console-display">
    <div class="display-label"><span>{stages[selected].tag}</span><span>{isZh ? '架构示意' : 'ARCHITECTURE STUDY'}</span></div>
    <div class="signal-wave" aria-hidden="true" style={`--signal-offset: ${selected * 18}deg`}>
      {#each bars as height, i}<i style={`--height:${height}%;--delay:${i * -35}ms`}></i>{/each}
    </div>
    <div class="display-scale" aria-hidden="true"><span>IN</span><span>ASYNC / BUFFERED</span><span>OUT</span></div>
  </div>
  <div class="console-stages" role="group" aria-label={isZh ? '探索音频路径' : 'Explore the audio path'}>
    {#each stages as stage, i}
      <button type="button" class:active={selected === i} aria-pressed={selected === i} onclick={() => selected = i}>
        <span class="stage-number">0{i + 1}</span><span>{stage.name}</span><i aria-hidden="true"></i>
      </button>
    {/each}
  </div>
  <div class="console-description" aria-live="polite" aria-atomic="true">
    <h2>{stages[selected].title}</h2><p>{stages[selected].body}</p>
    <span class="console-detail">{stages[selected].detail}</span>
  </div>
  <div class="console-bottom"><span>RUST → WEB AUDIO</span><span>{isZh ? '选择上方节点，探索链路' : 'SELECT A STAGE TO EXPLORE'} ↗</span></div>
</div>

<style>
  .signal-console { color: #ebe5d8; background: #292b28; border: 1px solid #4b4d47; border-radius: 8px; padding: 20px; box-shadow: 0 28px 60px #161a1626, inset 0 1px #ffffff17; transform: rotate(-1deg); }
  .console-top, .console-bottom, .display-label, .display-scale { display: flex; align-items: center; justify-content: space-between; gap: 12px; font: 10px/1.4 var(--font-mono); }
  .console-top { padding: 0 2px 18px; }
  .console-brand { font-weight: 800; font-size: 15px; letter-spacing: .03em; }
  .console-brand span { color: #a8ada1; font-size: 10px; font-weight: 400; }
  .console-id { color: #a8ada1; }
  .console-display { padding: 16px; background: #151c18; border: 1px solid #495045; border-radius: 3px; box-shadow: inset 0 3px 12px #0006; }
  .display-label { color: #aab7a4; font-size: 9px; }
  .signal-wave { display: flex; align-items: center; gap: 3px; height: 178px; position: relative; margin: 10px 0; background: repeating-linear-gradient(0deg, transparent 0 35px, #bbd5b70c 35px 36px), repeating-linear-gradient(90deg, transparent 0 35px, #bbd5b70c 35px 36px); filter: hue-rotate(var(--signal-offset)); }
  .signal-wave::before { content: ''; position: absolute; width: 100%; height: 1px; background: #b9d5a62b; }
  .signal-wave i { display: block; flex: 1; height: var(--height); background: #b9d59e; border-radius: 2px; box-shadow: 0 0 8px #b9d59e20; }
  .display-scale { color: #8d9c86; font-size: 9px; }
  .console-stages { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 7px; margin-top: 17px; }
  button { display: grid; grid-template-columns: 1fr auto; gap: 8px; text-align: left; cursor: pointer; border: 1px solid #4e514a; border-radius: 3px; background: #343731; color: #d3d7cc; padding: 12px; font: 600 12px/1.2 var(--font-sans); transition: background .15s, border-color .15s; }
  .stage-number { grid-column: 1 / -1; color: #a8ada1; font: 10px var(--font-mono); }
  button i { width: 5px; height: 5px; border-radius: 50%; background: #71756a; align-self: center; }
  button:hover { background: #41443d; }
  button.active { background: #454036; border-color: #bc9670; color: #f4dec2; }
  button.active i { background: #dcb188; box-shadow: 0 0 7px #dcb18855; }
  button:focus-visible { outline: 2px solid #dcb188; outline-offset: 3px; }
  .console-description { min-height: 159px; padding: 23px 2px 18px; }
  .console-description h2 { font: 600 17px/1.35 var(--font-sans); margin: 0 0 8px; color: #ebe5d8; }
  .console-description p { font-size: 13px; line-height: 1.65; color: #b4b9ad; max-width: 46ch; margin: 0 0 12px; }
  .console-detail { font: 10px/1.5 var(--font-mono); color: #d2ad88; }
  .console-bottom { border-top: 1px solid #494c43; padding: 14px 2px 0; color: #a8ada1; font-size: 9px; }
  @media (prefers-reduced-motion: no-preference) { .signal-wave i { animation: signal-breathe 2.4s ease-in-out infinite alternate; animation-delay: var(--delay); } @keyframes signal-breathe { from { transform: scaleY(.6); } to { transform: scaleY(1); } } }
  @media (max-width: 680px) { .signal-console { transform: none; padding: 13px; } .signal-wave { height: 150px; gap: 2px; } button { padding: 10px 7px; font-size: 11px; } .console-bottom { flex-wrap: wrap; } }
</style>
