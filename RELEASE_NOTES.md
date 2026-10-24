# MDcard 0.1.1 · 2026-10-24

Rust 原生 Markdown 与文本桌面编辑器。面向中文技术写作，提供编辑、分屏与阅读视图，结构编辑、搜索、公式图表与多格式导出。

本次 Windows 为 Icon Suite R3 (`icon-suite-20261024-r3`)，Linux 为 R29 (`precision-b-20261024-r29`)；两个平台构建身份不同，请按平台选择完整包。

## 安装

Windows x86_64：完整解压 ZIP，运行目录内 MDcard.exe；请保留所有伴随资源。

Linux x86_64：当前二进制基于 Debian 12 / glibc 2.36，已验收 X11 桌面。完整解压 tar.gz 后运行 `chmod +x mdcard && ./mdcard`。Debian/Ubuntu 运行依赖：

```sh
sudo apt-get install libxkbcommon-x11-0 libgl1 libegl1 libxcursor1 libxrandr2 libxi6 libvulkan1 mesa-vulkan-drivers zenity xdg-desktop-portal xdg-desktop-portal-gtk
```

## 边界

未提供 macOS 正式包。当前不含自动保存与崩溃恢复；正常关闭提示、原子保存、外部修改检测不能替代备份。源码在本机处理；HTTP(S) 图片与点击外链可能访问网络。完整包含本地 Node、Chromium、Java、Pandoc 与字体资源，约 576 / 626 MiB，不宣传极小包体。

下载后请核对 SHA256SUMS.txt。许可证与第三方声明随完整包提供。
