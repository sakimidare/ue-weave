# 参与开发

开发环境和模块说明见 [docs/development.md](docs/development.md)。

- 代码注释、API 文档和用户消息统一使用中文；类型名、函数名和协议名称保持英文。
- 对外 API 写明输入、输出和副作用，示例放入文档测试或集成测试。库启用 `missing_docs` 检查。
- 新增错误、警告或提示时，使用对应枚举并分配独立编号，同步补充 `--explain` 的解释和例子。
- 诊断详情使用 `diagnostic::Details`，新增字段时明确字段类型。
- 代码由 rustfmt 格式化。较长的 JSON 对象按字段换行，字符串按语义分段。
- 测试使用本地 fixture 和本地服务。命令行测试共用临时目录及命令构造工具。
- 按功能组织提交，提交前检查差异；验收输出放在忽略目录中。

提交前运行：

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo test --doc
cargo doc --no-deps
cargo build --release
```
