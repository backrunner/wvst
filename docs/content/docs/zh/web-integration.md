---
title: Web 接入
description: 在 Vite 应用中建立已授权的控制与音频连接，挂载一个立体声效果器，并完整释放资源。
order: 5
---

# Web 接入

本页把一个本地立体声 VST3 效果器接入应用已有的 `AudioContext`。先完成[快速开始](/docs/zh/getting-started)，确认 Bridge 和插件能在 Studio 中工作，再排查自己的集成。

## 打包与浏览器前置条件

在仓库运行 `npm run build:web`，让应用通过 workspace 依赖使用 `@wvst/web`。该包目前标记为 private，不要把 `npm install @wvst/web` 当作已发布的安装路径。

示例使用 Vite 的 `?worker` 和 `?url` 导入语法；TypeScript 项目需要 `vite/client` 类型。其他打包器应把 Worker 编译成独立模块，并将 AudioWorklet 入口作为可访问的 JavaScript URL 发布。不要在服务端渲染期间创建 Worker 或 AudioContext。

页面需具备安全上下文、AudioWorklet、SharedArrayBuffer 和跨源隔离。`requireLowLatency: false` 只跳过 SDK 的前置检查，不会提供没有 SAB 的音频替代路径。响应头配置见[配置与部署](/docs/zh/configuration)。

## 先让用户选择插件

用 `client.plugins.list({ rescan: true })` 获取 `plugins` 与 `failures`，展示插件及其 class，取得用户选择的 `pluginId`、`classId`。不要直接假设 `plugins[0]` 或 `classes[0]` 存在。

如果扫描元数据没有 class ID，使用 `client.plugins.factoryInfo({ path })` 查询，再让用户选择音频组件 class。插件被扫描到并不保证支持 2-in/2-out；挂载失败时保留错误信息让用户换一个兼容效果器。

## 挂载一个效果器

下面可保存为 Vite 应用的 `mount-effect.ts`。传入的 source 和 AudioContext 由应用持有；helper 拥有自己创建的连接、实例和 worklet。调用前确保 source 没有直接连到 destination，否则会同时听到原音和湿声。

控制 socket 和音频 Worker socket 分别执行 `bridge.hello`。`createHelloRequest()` 不保留连接时的 token，因此必须显式补上 `token`。

```ts
import {
  WVSTClient,
  WVSTBridgeWorkerClient,
  configureLoopbackAudioWorkletNode,
  createLoopbackSharedBuffers,
  readLoopbackMetrics,
  type InstanceDescriptor
} from '@wvst/web';
import BridgeWorker from '@wvst/web/bridge-worker?worker';
import processorUrl from '@wvst/web/loopback-processor?url';

const workletModules = new WeakMap<AudioContext, Promise<void>>();

export async function mountEffect(
  context: AudioContext,
  source: AudioNode,
  options: { pluginId: string; classId: string; token: string; endpoint?: string }
) {
  const endpoint = options.endpoint ?? 'ws://127.0.0.1:35876';
  let client: WVSTClient | undefined;
  let worker: Worker | undefined;
  let transport: WVSTBridgeWorkerClient | undefined;
  let instance: InstanceDescriptor | undefined;
  let node: AudioWorkletNode | undefined;
  let connected = false;
  let disposePromise: Promise<void> | undefined;

  function dispose(): Promise<void> {
    return disposePromise ??= (async () => {
      const errors: unknown[] = [];
      const attempt = async (operation: () => unknown) => {
        try { await operation(); } catch (error) { errors.push(error); }
      };
      if (connected && node) await attempt(() => source.disconnect(node!));
      if (node) {
        await attempt(() => node!.disconnect());
        node.port.close();
      }
      if (instance) {
        const { instanceId, streamId } = instance;
        if (transport) await attempt(() => transport!.stopAudioStream(streamId));
        if (client) {
          await attempt(() => client!.instances.stop({ instanceId }));
          await attempt(() => client!.instances.closeStream({ instanceId }));
          await attempt(() => client!.instances.destroy({ instanceId }));
        }
      }
      if (transport) await attempt(() => transport!.close());
      worker?.terminate();
      client?.close();
      if (errors.length) throw new AggregateError(errors, 'WVST cleanup failed');
    })();
  }

  try {
    client = await WVSTClient.connect({
      endpoint, token: options.token, requireLowLatency: true
    });
    worker = new BridgeWorker();
    transport = new WVSTBridgeWorkerClient({ worker });
    await transport.connect(endpoint);
    await transport.request('bridge.hello', {
      ...client.createHelloRequest().params,
      token: options.token
    });

    instance = await client.instances.create({
      pluginId: options.pluginId,
      classId: options.classId,
      sampleRate: Math.round(context.sampleRate),
      maxBlockFrames: 128,
      inputChannels: 2,
      outputChannels: 2
    });
    await client.instances.start({ instanceId: instance.instanceId });

    const buffers = createLoopbackSharedBuffers({
      frames: 128, inputChannels: 2, outputChannels: 2, capacityQuanta: 4
    });
    let module = workletModules.get(context);
    if (!module) {
      module = context.audioWorklet.addModule(processorUrl);
      workletModules.set(context, module);
      void module.catch(() => workletModules.delete(context));
    }
    await module;
    node = new AudioWorkletNode(context, 'wvst-loopback', {
      numberOfInputs: 1, numberOfOutputs: 1, outputChannelCount: [2],
      channelCount: 2, channelCountMode: 'explicit'
    });
    configureLoopbackAudioWorkletNode(node, buffers);
    await transport.startAudioStream({
      streamId: instance.streamId, sampleRate: instance.sampleRate,
      frames: 128, inputChannels: 2, outputChannels: 2, buffers
    });
    source.connect(node);
    connected = true;
    node.connect(context.destination);

    return { instance, readMetrics: () => readLoopbackMetrics(buffers), dispose };
  } catch (error) {
    try { await dispose(); }
    catch (cleanupError) {
      throw new AggregateError([error, cleanupError], 'WVST setup and cleanup failed');
    }
    throw error;
  }
}
```

## 播放与资源所有权

在用户点击播放的事件中先调用 `context.resume()`，再播放媒体或启动音源，以满足浏览器自动播放策略。这个 helper 不创建媒体元素、不请求麦克风权限，也不关闭应用共享的 AudioContext。

把 `mountEffect()` 返回值保存在组件中：

```ts
const effect = await mountEffect(context, source, {
  pluginId: selectedPluginId,
  classId: selectedClassId,
  token: tokenFromUser
});

// 在 UI 定时器中读取，不要在 AudioWorklet.process() 中记录日志。
console.table(effect.readMetrics());

// 移除效果器或离开页面时执行，并处理可能的远端清理错误。
await effect.dispose();
```

本段延续上面的 helper；`context`、`source`、选择结果和 token 由应用提供。首次尝试可用 Studio 的内置音频确认链路，再接自己的媒体输入。

| 资源 | 应用需要负责的事情 |
| --- | --- |
| AudioContext | 在用户操作中恢复；只在整个音频会话结束时关闭。 |
| 媒体元素与 source | 每个媒体元素只创建一次 `MediaElementAudioSourceNode`，重连时复用。 |
| 文件 URL | 替换文件或卸载时调用 `URL.revokeObjectURL()`。 |
| 效果器会话 | 挂载失败会清理部分资源；正常卸载调用并等待 `dispose()`。 |
| 事件订阅、计时器 | 取消订阅并停止计时器，避免组件销毁后继续更新。 |

示例中的 `dispose()` 可以重复调用，并尝试执行所有清理步骤，最后聚合报告失败。如果 Bridge 已退出，远端请求可能失败，应用仍应移除本地音频连接。页面在挂载期间销毁时，等待挂载结束后立即释放返回的会话，并禁止并发挂载操作。

## 多效果器与重连

这个 helper 是单效果器教学入口。机架应共享一个控制 client 和一个传输 Worker，每个 slot 只持有自己的 instance、buffers、node 和 stream。按照顺序构建 `source → effect A → effect B → destination`，变更前断开旧边，避免重复连接。

旁路应把节点从信号路径中移出；这不一定停止插件 worker，也不表示节省其全部 CPU。移除才执行实例与流释放。Studio 的 `rack-demo/audio-graph.ts` 和 `WVSTRackDemo.svelte` 提供了完整例子。

连接关闭后建立新的控制 client 和 Worker，并重新握手；不要继续使用旧 socket 或假设旧实例可直接复用。应用如需恢复参数，应预先保存快照，并检查插件、class 和处理配置兼容性。

## 声道、时序和指标

本例固定 128 帧、2-in/2-out，并使用 AudioContext 的实际采样率。四个 quantum 是环形缓冲的容量，不等于固定延迟或延迟补偿。48 kHz 下 128 帧约为 2.67 ms，一次完整往返还包括调度、传输、排队和插件本身延迟。

`readMetrics()` 返回队列、underflow/overflow、事件丢弃和传输失败计数，不直接返回端到端延迟。比较一段时间内的增量，避免把启动瞬间的一次欠载误判为持续故障。详细指标来源见[API 参考](/docs/zh/api-reference)。

音源需要匹配插件的零输入或其他总线布局，并发送带采样偏移的 MIDI 事件；不要只把效果器示例的输入声道改为零后就假定能发声。参考仓库 `packages/wvst-web-examples/src/instrument` 和[故障排查](/docs/zh/troubleshooting)。
