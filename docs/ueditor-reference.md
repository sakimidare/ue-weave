# UEditor 规则来源

上游仓库：https://github.com/fex-team/ueditor

参考提交：`ea2585b483b9d3ead85b090928d72503c7107baf`。

`vendor/ueditor` 是 Git 子模块，提交由主仓库的 gitlink 固定。上游许可证见该目录的 `LICENSE`，项目说明见 `README.md`。

规则参考范围包括 Editor、htmlparser、filternode、filterword、dtd、domUtils、paste 和 parse。工具使用 Rust 函数解析静态 HTML，vendor 中的 JavaScript 用于源码查阅。Cargo 发布包通过 `exclude` 排除 vendor 和验收产物。
