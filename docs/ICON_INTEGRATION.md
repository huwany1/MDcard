# 图标接入与维护

`src/branding.rs` 使用 include_bytes 嵌入黑/白 16、24、32、48px 与彩色 256px PNG；按实际背景亮度选择单色版本。Windows build.rs 将彩色 ICO 编译为 PE 主图标及版本资源（MSVC rc.exe / GNU windres）；Linux 跳过 PE 步骤。

资源损坏时回退到内建几何并报告 fallback；图标不依赖运行目录。Windows AppUserModelID 为 MDcard.Desktop，快捷方式可用 `scripts/Create-MDcardShortcut.ps1 -AppDirectory <完整应用目录>` 显式创建或修复。移动程序后应从新目录重建入口。

`assets/branding/mdcard-icon/` 保留可编辑主 SVG、小尺寸光学校正 SVG、完整 PNG/ICO 套装与生成/预览/验证工具；其用途是维护品牌资源和网站，不只是运行时引用。预览和生产检查结果为忽略的本地生成物。重建步骤与视觉约束见[设计说明](../assets/branding/mdcard-icon/DESIGN.md)。

历史 Windows 11 / 125% DPI 的显示验收不覆盖其他 DPI、macOS、Wayland 或当前新二进制；需要真实窗口/任务栏/快捷方式验证时重新记录环境。
