# 大型音源与 Web MIDI 边界审查（2026-09-08）

## 结论

进程隔离、非阻塞 AudioWorklet、有限音频缓冲和 MIDI 到 VST3 的基础链路已经存在，但此前不能称为对 Kontakt 这类大型采样音源处理完整。本轮修复了可复现的加载预算、运行期失败计数、资源并发限额、迟到输出和 MIDI 生命周期问题。没有把测试插件的成功当作 Kontakt 兼容性认证。

## 本轮修改

| 场景 | 原问题 | 修复与验证 |
| --- | --- | --- |
| 慢加载 / 恢复大型预设 | `instance.create`、state/preset 写入与音频/心跳共用 5 秒 IPC 超时 | 独立 `worker_load_timeout`，默认 120 秒；`WVST_WORKER_LOAD_TIMEOUT_MS` / `BridgeConfig::with_worker_load_timeout()` / `WorkerSupervisorOptions::with_load_timeout()` 可配置。隔离 factory metadata probe 也采用加载预算。普通 IPC 仍为 5 秒；单次请求整体预算覆盖 write/header/body，不能每读一段重新延长总截止时间。 |
| 永久卡住 | 已有超时后终止进程树和回收子进程 | 保留机制，新增真实子进程睡眠 fixture，检查 load timeout、shutdown audit 和实例额度释放。 |
| 浏览器连接不关闭但不响应 | 未收到 close/error 时请求可永久挂起，无待处理数量上限 | WebSocket 握手默认 10 秒、控制请求 180 秒、音频响应 10 秒；待处理请求最多 256 个。任一请求超时关闭该连接并拒绝所有待处理请求，避免 FIFO 二进制响应错配。可经 `transportOptions` 配置；这些是故障截止时间，不是实时延迟指标。 |
| 同时加载多个大型实例 | 数量检查只统计已 ready 的 worker，并发启动可越过上限 | spawn 前原子预留 semaphore 额度，正在加载的实例也计数；失败/销毁释放额度。 |
| 能加载但运行后反复崩溃 | 成功 create 清空失败次数，运行期死亡不计入 quarantine | 移除失败 worker 时只计数一次；加载成功不清空失败。相邻失败间隔达到 60 秒后重新计数，默认 3 次进入 60 秒 quarantine；统一计数与隔离锁，消除锁顺序反转。 |
| 插件 stderr 巨长且不换行 | 按行读取在截取 tail 前可积累无界单行 | 1024 字节有界读取、最多保留 4096 字节 tail；百万字节无换行 fixture 验证。 |
| 可选接口不支持 / 普通控制拒绝 | runtime snapshot 查询不支持的 unit metadata 会失败；普通控制拒绝会杀死可工作的插件 | `units` 能力不可用时返回 null，保留能力诊断；有效控制拒绝返回给调用方，保留 worker。运行期 DSP 失败仍按致命故障处理。VestiMIDISynth 的 `IUnitInfo` 返回 -1 场景及重复控制/batch 拒绝已验证。 |
| DSP 长时间超预算 / 重启音频流 | 已返回的旧结果仍可能播出；已停止的 pump 可能向重用的 SAB 写入 | DedicatedWorker 按输入块序号拒绝超出 capacityQuanta 窗口的结果，累加丢弃/欠载计数；停止后忽略在途结果。 |
| 错误/损坏音频输出 | `PROCESS_ERROR` 被当普通静音；未完整核对响应归属 | 检查 sequence、sample time、sample rate、格式/长度与非有限采样值；处理错误可观测，输出静音；LATE 丢弃。 |
| 过载跳过旧输入块 | 旧 MIDI 连同 Note Off、踏板松开一起丢弃，可能挂音 | 迟到 MIDI 按原块和 sample offset 顺序移至下一处理块的 offset 0；计入 late，不计为 dropped。参数自动化仍按原策略丢弃过期事件。入队先整体校验，避免半个批次留下 Note On。 |
| Web MIDI 热插拔 / 多输入 | 仅枚举一次输入，连接后插入设备无效 | 订阅 `statechange`，去重绑定、拔出解绑，可通过 `inputIds` 筛选。 |
| MIDI 断开 / 停止 / 页面失焦 | 无设备级 note/pedal 释放 | adapter 追踪音符与 CC64/66/69，拔出自动释放，提供 `panic()`；`stop()` 返回 Promise，应在关闭音频流前 await。instrument 示例接入释放和错误展示。 |
| 短消息损坏 / SysEx | 补零接受不完整消息、长消息静默截断为 3 字节；同步解析异常漏出 | 校验状态、完整长度和数据字节，明确拒绝 SysEx/拼接消息；解析和发送异常送入 `onError`。 |

WebSocket 与 native audio TCP 连接启用 `TCP_NODELAY`，避免小包受到 Nagle 合并策略影响。没有把此次 smoke 的表现归因于这一项改动；最初无响应的直接原因是 smoke 音频连接缺少 `bridge.hello` 授权，已经补齐。启动日志按完整行提取地址，避免把分块输出中的 `ws://127` 当作完整 URL。

变更位于 Bridge/控制线程及浏览器 DedicatedWorker；没有向 AudioWorklet 的 `process()` 添加工作。输出有限值检查增加 DedicatedWorker 每块一次有界 O(samples) 扫描；其成本需与端到端 smoke 结果一起评估，不代表第三方重型音源性能达标。

## 已有能力与仍未完成的边界

- **崩溃隔离**：默认每实例独立进程，Bridge 不加载插件；音频缺失时 Worklet 返回静音并计数。无法据此承诺内存耗尽时整个操作系统不受影响。
- **内存/CPU**：已有可选 address-space / 累计 CPU 时间限制，以及 Linux cgroup CPU/memory 限制，默认未启用。macOS 的地址空间上限不是 RSS 限额；CPU 秒数不是实时 CPU 百分比。尚无持续 RSS/CPU 采样、全局内存压力预算或按实际 DSP 负载自动降级机制。
- **加载体验**：目前是等待 create 的异步请求，没有取消正在创建的插件、采样库加载进度或后台磁盘预载完成信号。加载期间同一控制连接的后续请求和事件推送仍等待当前请求完成；其他连接和实例独立运行。不能把 VST3 initialize 返回当作音色采样已全部预载完成。控制与音频应使用独立连接；同一实例恢复 state 前应暂停音频处理，避免其长时间占用 worker 时触发音频超时。增大原生加载预算时需同步给浏览器控制预算留足余量；浏览器超时断开不取消原生加载，也不销毁可能已创建的实例。
- **恢复**：`instance.status` 的心跳检查可触发恢复，音频处理错误也会标记失败；没有持续后台 watchdog 主动轮询所有空闲实例。Web session 自动重启的是音频传输，不能等同于重建原生插件。原生重启不自动恢复 opaque state / 音色 / 参数 / 持续音符，应用需显式保存并恢复 state。
- **实时性能**：有界缓冲和丢弃策略防止无界积压，不会让算不完的插件实时运行。持续超载仍产生断音；增加缓冲会增加延迟。尚未证明 Kontakt 在 48 kHz / 128 frames / 4–8 quanta 下长时间稳定。
- **MIDI 支持**：note on/off、velocity-zero note-off、poly pressure 进入 VST3 IEventList；CC（含踏板）、pitch bend、channel aftertouch 通过插件 `IMidiMapping` 映射参数。插件不提供映射时对应控制可能被忽略；事件输入 bus 当前固定为 0。
- **MIDI 未覆盖**：Program Change 与系统实时消息可由短消息 raw 格式传输，但当前 native 输入转换不实现对应 VST3 动作；SysEx 明确不支持。adapter 的 `timeStamp` 不自动换算音频时钟，sampleOffset 默认 0，可显式指定。MIDI out 到硬件、真实 MIDI 键盘时钟校准未覆盖。
- **极端事件洪泛**：队列容量有界、满时拒绝并计数。迟到事件保护不保证队列满/传输失败时零丢事件；应用需监听 `onError`，必要时 panic 或重建实例。native shared-memory pump 的显式 `drop-oldest` 策略仍会丢事件，演奏建议保留默认 `reject`。
- **多设备同音符**：设备拔出时会检查其他设备仍持有的同音符/踏板；普通消息仍直接转发，没有多设备演奏的完整 voice 合并规则。追踪集合不区分同一设备同音符的重复 Note On；释放失败会通知 `onError`，但已清空追踪，不能承诺重试 panic 即可消除所有挂音。
- **释放确认**：`await adapter.stop()` 等待目标的发送确认；内置 Worker 只确认入队，不确认 native 已处理。立即关闭音频流可能丢弃释放事件，当前无公开 MIDI flush 确认。需保持处理运行让释放到达，结束会话还应停止/销毁原生实例。最终 smoke 特意保持音频运行，并等待输出缓冲全部更新后验证归零。
- **UI**：文档站 Live Studio 是立体声效果器机架，未接 MIDI 音源面板；MIDI 输入由 SDK 和独立 instrument 示例提供。

## 验证记录

- Rust 子进程回归：慢加载/慢 state、心跳预算、永久卡住、并发启动限额、运行后重复崩溃、无换行 stderr 洪泛、普通控制拒绝不杀进程。
- Web 单元回归：热插拔、输入筛选、断开/stop/panic 释放、解析与异步发送错误、迟到 MIDI、批次原子校验、迟到/停止后音频响应、诊断错误、响应归属/格式和 NaN/Infinity 静音、停滞连接超时及有界请求队列。
- 本机 VST3：Reaktor 6、VestiMIDISynth、VestiGain、VestiWebUIDemo；没有 Kontakt。浏览器原生链路使用 VestiMIDISynth 测试插件，不能代表 Native Instruments 大型采样库兼容性，也不包含硬件 Web MIDI 授权或实际设备测试。
- `cargo test --all`：489 项通过；`cargo clippy --all-targets --all-features -- -D warnings`、`cargo fmt --all -- --check` 通过。
- `npm run check:web`、`npm run test:web`：10 个测试文件、49 项通过；Web SDK 构建通过。示例类型检查和文档检查也通过。

### 浏览器到原生音源的最终记录

证据：`output/evidence/instrument-web-midi-adapter.json`（本地生成的 evidence，不纳入源码）。Chromium headless，VestiMIDISynth，48 kHz / 128 frames / 0-in 2-out / capacity 8，运行 10 秒；输出增益为零，不向扬声器播放。

| 指标 | 结果 |
| --- | --- |
| Web MIDI 输入 | 合成 input 发出 `[0x90, 60, 100]`，经真实 adapter → DedicatedWorker → Bridge → VST3 |
| Note On 输出峰值 | 约 0.05，非静音 |
| adapter stop | 发出 Note Off、解绑 listener，无 adapter 错误；全部输出缓冲被后续块覆盖后 peak/rms 均为 0 |
| 已消费输出块 | 3686 |
| 欠载 / 丢输入块 / 丢输出块 | 8 / 0 / 0 |
| MIDI 丢弃 / 传输故障 / worker 故障 | 0 / 0 / 0 |
| Bridge route histogram | p50 ≤ 250 µs、p95 ≤ 1 ms、p99 ≤ 1 ms；这是 Bridge 路由耗时，不是键盘到扬声器延迟 |
| 浏览器计数轮询估值 | p50 12.705 ms、p95 24.935 ms、p99 25.650 ms；主线程轮询估计，无硬件时间戳，不作为精确端到端延迟 |

较早的 `instrument-edge-smoke.json` 同为 8 块缓冲，曾记录 89 次欠载、71 个丢输入块、6 个丢输出块；不同运行表现有差异。原 smoke 用输入/输出计数顺序配对，丢块后对应关系失效，旧报告中的高 round-trip 值不能视作真实延迟。本轮脚本遇到丢块或传输失败时将 `endToEndRoundTripUs.valid` 设为 false，并省略百分位。最终短测通过只证明功能链路和释放行为；尚未达到 30 分钟稳定性验证标准。

复现命令（从仓库根目录，需本机安装该 fixture）：

```sh
WVST_TOKEN=wvst-adapter-smoke \
WVST_BRIDGE_SMOKE_TOKEN=wvst-adapter-smoke \
WVST_BRIDGE_SMOKE_PLUGIN_PATH='/Users/orchiliao/Library/Audio/Plug-Ins/VST3/VestiMIDISynth.vst3' \
WVST_BRIDGE_SMOKE_INPUT_CHANNELS=0 \
WVST_BRIDGE_SMOKE_CAPACITY_QUANTA=8 \
WVST_BRIDGE_SMOKE_DURATION_MS=10000 \
WVST_BRIDGE_SMOKE_REQUIRE_NON_SILENT=true \
WVST_BRIDGE_SMOKE_VERIFY_MIDI_RELEASE=true \
WVST_BRIDGE_SMOKE_REPORT=output/evidence/instrument-web-midi-adapter.json \
npm run smoke:bridge:web
```

`WVST_BRIDGE_SMOKE_VERIFY_MIDI_RELEASE` 只用于无 release tail 的测试音源，不适合作为所有真实音源的通用静音断言；普通 instrument smoke 也经 Web MIDI adapter 输入，但默认不要求两秒内音尾归零。
