# Live Studio 翻新（2026-09-07）

用户要求全面优化 Demo 的样式、设计与引导。本轮延续现有纸色／石墨／铜色主题，将 `/demo` 和 `/zh/demo` 从文档文章容器迁移到 `studio` 自定义布局，保留 Svedocs Root 的导航、搜索、语言与主题上下文。

## 交互与职责

- `StudioGuide.svelte`：连接、选择音频、添加效果器三个步骤，完成状态来自真实页面状态，链接跳转到对应区域。
- `ConnectionPanel.svelte` / `BridgeSetup.svelte`：首访安装指引；连接后收起帮助；高级地址、Token 与浏览器诊断按需展开，Token 为密码输入；源码命令支持复制。
- `PlayerDeck.svelte`：真实文件波形、进度、播放/暂停、音量、静音、循环、拖放和合成音频入口。
- `audio-file.ts`：本机生成 8 秒 PCM WAV 合成循环；用 OfflineAudioContext 读取音频波形；文件超过 50 MB 时跳过额外解码，仍可用 media element 播放。
- `audio-graph.ts`：从原先超过 600 行的 Demo 组件抽出 AudioContext、媒体源、worklet 创建、routing 和电平管理。重连复用媒体源，卸载时释放。显式双声道 GainNode 让单声道输入也能正确送往立体声输出。
- `EffectRack.svelte`：区分未连接、已连接无插件、可添加、已添加与旁路状态；参数默认展开；处理计数与插件延迟收进详情。
- `StudioFooter.svelte`：实际信号链、双声道输出电平、常见问题。效果器名称与状态都来自当前实例，不提供模拟处理 UI。
- CSS 按 shell、player、rack 分开；中英文文案集中在 `copy.ts`。

原始音频可以在连接前试听，页面明确标记原始音频或启用效果器。未修改 AudioWorklet realtime process。播放失败现在显示可读错误，重连与销毁继续沿用上一轮资源清理修复。

## 同时修复

旧 Demo 只在控制 WebSocket 上执行 hello，DedicatedWorker 的独立音频 WebSocket 没有授权。Bridge 会忽略未授权的二进制帧。本次在 worker connect 后为音频连接执行带 Token 的 `bridge.hello`，再完成 UI 连接流程。浏览器回归验证两个连接均授权，并且实际交换二进制音频帧。

## 验证

- Svedocs：16 页、129 条搜索记录，0 errors；保留已有的未配置 site.url 警告。
- Svelte/TypeScript：0 errors、0 warnings。
- 生产构建成功，包括 16 个 OG SVG 与 Cloudflare adapter 输出。
- Web SDK：7 个测试文件，27 项通过。
- 新增 `npm --workspace @wvst/docs run smoke:studio`：使用 Playwright 与独立的本地协议 fixture，验证首访、合成音频、波形、两个 socket 的 Token 握手、空扫描、失败实例清理、参数编辑、多个实例排序、旁路/恢复、移除、断开重连播放、深色主题、390px 移动端和中文页面；页面异常为 0。

此回归使用 echo 音频的测试 fixture，不代表真实第三方 VST3 音质、兼容性或长时间稳定性验证。fixture 仅在测试脚本启动，不进入产品 bundle。
