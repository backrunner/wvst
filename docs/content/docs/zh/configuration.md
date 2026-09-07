---
title: 配置与部署
description: Bridge 环境变量、token 与 origin 策略、浏览器隔离响应头，以及文档站部署检查。
order: 7
---

# 配置与部署

WVST 有两种部署对象：供浏览器访问的网页，以及运行在使用者电脑上的 Bridge 和 host worker。托管文档站不会安装、启动或远程替代用户的本地插件运行时。

## 本地授权配置

独立 CLI 的 `serve` 要求非空 `WVST_TOKEN`，每个 WebSocket 都需完成带该 token 的 `bridge.hello`。网页主线程和音频 Worker 的 socket 分别授权。嵌入式 `BridgeConfig::development()` 可以采用无 token 配置，这不代表 CLI 的默认行为。

下面的本地开发配置只允许一个精确网页 origin：

```sh
WVST_TOKEN=local-dev-token \
WVST_ALLOWED_ORIGINS=http://localhost:5173 \
WVST_ALLOW_LOOPBACK_ORIGINS=false \
WVST_HOST_WORKER=target/debug/wvst-host-worker \
  target/debug/wvst-bridge-server serve
```

打开 `http://localhost:5173` 并在 Studio 输入同一个 token。`http://127.0.0.1:5173` 是另一个 origin，端口变化也会改变 origin。

`WVST_ALLOWED_ORIGINS` 是额外允许的精确值；默认仍自动允许 loopback 网页。只有同时设置 `WVST_ALLOW_LOOPBACK_ORIGINS=false`，才会关闭这条自动允许规则。填写网页 origin（协议、主机、端口），不要填写 `/demo` 路径或 Bridge 的 WebSocket URL。

Origin 检查针对浏览器来源；无 Origin 的原生客户端不被这条策略阻止，因此它不能替代 token。当前 CLI 使用配置的共享 token，不提供完整的短期配对码、用户账户或浏览器自动领证流程。更换 token 后重启 Bridge，并让客户端重新连接。

## Bridge 环境变量

配置在启动时读取。排查时用相同的环境运行 `diagnose`；修改另一个 shell 的变量不会改变已经运行的进程。

| 变量 | 默认值 | 说明 |
| --- | --- | --- |
| `WVST_TOKEN` | 无；CLI 启动时必填 | 非空会话 token，不写入公开前端配置。 |
| `WVST_BIND_ADDR` | `127.0.0.1:35876` | 监听地址和端口；网页 endpoint 必须匹配。保持本地 loopback 部署。 |
| `WVST_HOST_WORKER` | 自动发现 | 可执行文件路径；从其他工作目录或服务启动时建议使用绝对路径。 |
| `WVST_ALLOWED_ORIGINS` | 空列表 | 逗号分隔、精确匹配的额外 origin。 |
| `WVST_ALLOW_LOOPBACK_ORIGINS` | `true` | `0` 或 `false` 关闭自动允许本地网页。 |
| `WVST_WORKER_AUTO_RESTART` | `true` | `0` 或 `false` 关闭 worker 自动重启。 |
| `WVST_MAX_WORKER_INSTANCES` | `64` | 并发 worker 上限，不是可实时承载 64 个插件的性能保证。 |
| `WVST_WORKER_QUARANTINE_FAILURES` | `3` | 插件进入隔离前的失败阈值。 |
| `WVST_MAX_CONTROL_MESSAGE_BYTES` | `16777216`（16 MiB） | JSON 控制消息大小上限；大状态快照也受影响。 |
| `WVST_WORKER_MEMORY_LIMIT_BYTES` | 未设置 | 支持的平台上限制 worker 地址空间，不等同于常驻内存指标。 |
| `WVST_WORKER_CPU_TIME_LIMIT_SECONDS` | 未设置 | 支持的平台上限制累计 CPU 时间，不是单次 process 的超时。 |
| `WVST_WORKER_LINUX_CGROUP_PARENT` | 未设置 | 可写的 Linux cgroup 父目录。 |
| `WVST_WORKER_LINUX_CGROUP_MEMORY_MAX_BYTES` | 未设置 | cgroup 内存上限，需要 cgroup 配置。 |
| `WVST_WORKER_LINUX_CGROUP_CPU_QUOTA_MICROS` | 未设置 | cgroup CPU 配额，单位微秒。 |
| `WVST_WORKER_LINUX_CGROUP_CPU_PERIOD_MICROS` | `100000` | cgroup CPU 配额周期，单位微秒。 |

若插件频繁失败，先查看事件和 worker 诊断，再决定是否调整限制。提高上限无法修复插件的架构或总线不兼容。

## 浏览器隔离与资源

低延迟页面需运行于 HTTPS 或被浏览器信任的 localhost，并返回：

```text
Cross-Origin-Opener-Policy: same-origin
Cross-Origin-Embedder-Policy: require-corp
```

从最终页面响应验证，而不只是检查服务器配置文件：

```sh
curl -I http://localhost:4173/zh/demo
```

在控制台确认 `isSecureContext`、`crossOriginIsolated` 和 `typeof SharedArrayBuffer === 'function'`。COEP 会影响第三方图片、字体、脚本和嵌入内容；资源需同源或提供适当的 CORS/CORP 响应。Worker 与 worklet URL 应返回 JavaScript，不能被 SPA fallback 改写成 HTML。

网页的隔离状态与能否访问本地 WebSocket 是两个检查点。远程 HTTPS 页面访问 `ws://127.0.0.1:35876` 还受浏览器混合内容、本地网络访问权限和企业策略影响。Bridge CLI 提供的是 `ws://`，不能只把 URL 改为 `wss://`；需要 TLS 的部署必须另行提供受信任的本地 TLS 终结。先用 localhost 跑通，再验证目标浏览器中的远程部署。

## 文档站构建

在仓库根目录：

```sh
npm ci
npm run docs:check
npm run docs:build
npm --workspace @wvst/docs run preview
```

默认构建使用 Cloudflare adapter；本地 preview 的输出地址通常为 `http://localhost:4173`。需要静态站时：

```sh
npm run build:web
npm --workspace @wvst/docs run build:static
```

静态 adapter 默认输出 `docs/build`。不要将默认 edge 构建结果当作普通静态目录上传；部署方式应匹配 adapter。

| 运行方式 | 隔离响应头由哪里提供 |
| --- | --- |
| Vite 开发 | `docs/vite.config.ts` 中的 `server.headers`。 |
| 本地生产预览 | 同文件的 preview middleware。 |
| SvelteKit 动态响应 | `docs/src/hooks.server.ts`。 |
| Cloudflare 资产 | `docs/_headers` 的资产规则。 |
| 其他静态托管 | 在平台或反向代理上自行配置，静态页面不会执行 server hook。 |

准备正式域名后，在 `docs/svedocs.config.ts` 的 `site` 中设置实际 `url`，用于 canonical、sitemap 和 OG 地址。当前没有预设生产域名，因此构建会提示 `site.url` 未设置。

## 部署后验收

1. 打开英文和中文文档、首页及 Studio，确认刷新深层路径也能访问。
2. 检查页面响应头，以及 Worker、worklet、Logo 和搜索资源是否正常返回。
3. 在页面检查低延迟前置条件，再用本机 Bridge/token 连接。
4. 在 Bridge allowlist 中加入实际网页 origin；浏览器若请求本地网络权限，按实际部署要求处理。
5. 播放一个本地音频、挂载已知可用的效果器，再检查参数、旁路和指标。

详细文档维护方式见仓库 `docs/README.md`；发布包与真实插件验证见[开发与验证](/docs/zh/development)。
