import type { DemoCopy } from './types';

export const demoCopy: Record<'en' | 'zh', DemoCopy> = {
  en: {
    chooseFile: 'Choose audio', dropFile: 'Drop an audio file here', replaceFile: 'Replace audio', noFile: 'No audio loaded',
    connect: 'Connect', disconnect: 'Disconnect', scan: 'Rescan', scanning: 'Scanning plugins', add: 'Mount effect',
    play: 'Play', pause: 'Pause', stop: 'Stop', back: 'Back 10 seconds', forward: 'Forward 10 seconds', loop: 'Loop',
    mute: 'Mute', unmute: 'Unmute', endpoint: 'Bridge endpoint', token: 'Session token', tokenHint: 'Optional',
    settings: 'Connection settings', prerequisites: 'Browser readiness', secureContext: 'Secure context', isolation: 'Cross-origin isolation',
    sharedBuffer: 'SharedArrayBuffer', ready: 'Ready', missing: 'Missing', selectPlugin: 'Choose an effect VST',
    emptyRack: 'The rack is empty', emptyRackBody: 'Connect the bridge, choose an effect and mount it into the signal path.',
    noPlugins: 'No effect plugins found', connecting: 'Looking for the local Bridge', connected: 'Bridge connected', blocked: 'Low-latency prerequisites are missing.',
    bridgeError: 'Bridge connection failed.', fileReady: 'Audio is ready', invalidFile: 'Choose a supported audio file.',
    mounted: 'Effect mounted', removed: 'Effect removed', meters: 'Realtime metrics', bypass: 'Bypass', enable: 'Enable',
    remove: 'Remove', up: 'Move up', down: 'Move down', status: 'Runtime status', failures: 'Scan failures', dryPath: 'Dry path',
    processedPath: 'WVST path', volume: 'Output volume', parameters: 'Plugin parameters', noParameters: 'No editable parameters reported',
    inputQueue: 'Input', outputQueue: 'Output', underflows: 'Underflow', overflows: 'Overflow', latency: 'Latency',
    bridgePanel: 'Bridge and browser status', rackPanel: 'Effect rack'
  },
  zh: {
    chooseFile: '选择音频', dropFile: '拖入音频文件', replaceFile: '替换音频', noFile: '尚未加载音频',
    connect: '连接', disconnect: '断开', scan: '重新扫描', scanning: '正在扫描插件', add: '挂载效果器',
    play: '播放', pause: '暂停', stop: '停止', back: '后退 10 秒', forward: '前进 10 秒', loop: '循环',
    mute: '静音', unmute: '取消静音', endpoint: 'Bridge 地址', token: 'Session Token', tokenHint: '可选',
    settings: '连接设置', prerequisites: '浏览器就绪状态', secureContext: '安全上下文', isolation: '跨域隔离',
    sharedBuffer: 'SharedArrayBuffer', ready: '就绪', missing: '缺失', selectPlugin: '选择 Effect VST',
    emptyRack: '效果器机架为空', emptyRackBody: '连接 Bridge，选择效果器并把它挂载到信号链。',
    noPlugins: '没有找到效果器插件', connecting: '正在寻找本地 Bridge', connected: 'Bridge 已连接', blocked: '缺少低延迟前置条件。',
    bridgeError: 'Bridge 连接失败。', fileReady: '音频已就绪', invalidFile: '请选择浏览器支持的音频文件。',
    mounted: '效果器已挂载', removed: '效果器已移除', meters: '实时指标', bypass: 'Bypass', enable: '启用',
    remove: '移除', up: '上移', down: '下移', status: '运行状态', failures: '扫描失败', dryPath: 'Dry Path',
    processedPath: 'WVST Path', volume: '输出音量', parameters: '插件参数', noParameters: '插件未报告可编辑参数',
    inputQueue: '输入', outputQueue: '输出', underflows: '欠载', overflows: '溢出', latency: '延迟',
    bridgePanel: 'Bridge 与浏览器状态', rackPanel: '效果器机架'
  }
};
