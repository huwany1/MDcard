# 贡献指南

欢迎提交可复现问题和范围明确的 Pull Request。[问题入口](https://github.com/huwany1/MDcard/issues)用于缺陷、功能讨论和构建问题；请先搜索已有问题。

从公开 main 新建分支，说明问题、触发步骤、预期行为、修改范围和验证结果。修复文件读写、编码、撤销、阅读恢复、渲染或跨文件替换时，提供能够证明稿件没有意外改变的回归检查。不要提交真实私人稿件、个人设置、凭据、下载运行时或构建缓存。示例应为自己有权公开的最小内容。

提交前执行：

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --locked --release
```

改动 renderer 时按[开发说明](docs/DEVELOPMENT.md)执行 npm 干净安装、构建、真实引擎测试和进程取消测试；改动网站时执行其独立构建及受影响浏览器检查。UI、平台和性能结论必须记录实际环境，未运行检查标记未验证。独立 Rust 回归执行 `python scripts/check.py --acceptance`。原生 GUI 和性能检查的额外前提见[工具入口](scripts/README.md)，结果写入被忽略的本地目录。

全部自有贡献，包括代码、脚本、配置、文档、翻译、示例、图标和其他资产，统一使用 AGPL-3.0-only，不提供 MIT 备选授权；可加注释的源文件带 SPDX 标识，其他文件由根目录 [COPYRIGHT](COPYRIGHT) 覆盖。提交贡献表示你有权将贡献按该许可证提供。第三方代码和资产保留原版权、许可证、NOTICE 和来源，在[第三方清单](docs/THIRD_PARTY.md)中记录；自有修改与上游原内容的授权范围须明确区分。字体派生和子集继续遵守 OFL。详见[统一授权说明](docs/LICENSING.md)。

安全问题请避免在公开 Issue 中粘贴密钥、私人文件或可直接利用的敏感数据。先提供去除敏感数据的描述；维护者确认安全沟通方式后再传递必要详情。当前没有专门的私人安全报告通道或响应时限承诺。
