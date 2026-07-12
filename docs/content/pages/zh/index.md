---
title: WVST
description: 通过 Rust Bridge，把隔离的本机 VST3 插件接入真实 WebAudio graph。
layout: home
---

# WVST

WVST 让浏览器音频图可以连接本机 VST3 插件，同时不把第三方插件加载到浏览器或 Bridge Server 进程内。Web 负责界面与 routing，本地 Bridge 负责发现、隔离 worker 和原生处理。

先阅读[文档](/docs/zh)，或者在本地 bridge 已启动时打开 [live rack demo](/zh/demo)。
