# 开发说明

[返回首页](../README.md)

本文中的 IR 指程序内部及 JSON 输出使用的文章结构，DOM 指 HTML 解析后的节点树。

## 还原规则与报告

解析流程基于静态 HTML 节点和属性。格式来源包括语义标签、内联样式、UEditor 标记和 Word 粘贴残留。

- 提取段落、换行、列表、表格及文本格式。
- 识别 `sudyfile-attr`、`isupload` 和附件链接，清理附件图标。
- 将 `.wp_pdf_player` 的 `pdfsrc` 转换为 PDF 附件链接，使用属性中的文件标题；开启附件下载后使用本地文件路径。
- 处理 `undefinedundefined`、连续 NBSP、空块、`Mso*`、`mso-*` 和 Office 命名空间残留。
- 将加粗的中文编号短段落推断为标题，将末尾右对齐单位和日期推断为署名。
- 为缩进、标题、署名等推断记录 `value`、`confidence`、`reason`、`source_nodes`。
- 将原始节点、属性、文本和顺序记录在 `original_nodes` 中。

UEditor 的保存结果是 HTML。结构还原以这份 HTML 中的内容为依据；原始 Word 精确样式、编辑操作历史和已被过滤的属性记为未知项。网页中明确存在的字体、字号、颜色及间距写入 IR。

`--json` 的终端输出包含格式预设、开关和格式修改记录，以及推断、未知项和下载错误。结构化正文中的 `metadata.content_integrity` 记录正文文字及顺序的检查结果。

`metadata.resource_integrity` 记录还原前后图片、超链接和 PDF 地址的数量是否一致。

### 资源下载

默认请求公网地址。初始 URL、重定向目标和 DNS 返回的实际连接地址均参与校验；内部网站使用 `--allow-private-network`。请求直接连接目标，系统代理环境变量保持独立于此下载流程。

网页和单个资源的读取上限为 64 MiB。图片按 JPEG、PNG、GIF、WebP 文件签名保存；响应类型与签名冲突时记录下载错误。

附件文件名依次参考响应头 `Content-Disposition`（包括 UTF-8 `filename*`）、附件标题和最终 URL。中文名称保留，路径分隔符转换为普通字符。ZIP 文件可保留 DOCX、XLSX、PPTX、EPUB 等原来的扩展名，OLE 文件可保留 DOC、XLS、PPT 等扩展名；记录中的 `detected_type` 表示检测到的容器类型。其他数据文件保留原来的扩展名，内容类型记为 `unknown`。内容签名与扩展名冲突时调整扩展名，并写入处理记录；程序、脚本或 HTML 等可执行内容使用 `.download`。文件名和内容类型均未提供扩展名时使用 `.bin`。资源以普通数据文件权限创建，同名文件使用新文件名。

资源清单的 `download` 字段记录 `requested_url`、`final_url`、`status`、`content_type`、`size`、`sha256`、`local_path`，以及原始文件名、文件名来源、检测到的内容类型和命名调整。IR 再导出时核对已有 SHA-256。修改资源内容后，同时更新对应的资源记录。目录和符号链接的实际位置按资源根目录校验。

### 样式与工程检查

`style_registry.rs` 统一记录通用格式的 IR 目标、表格和单元格属性集合，供解析、审计与 HTML 导出共用。Markdown 的 HTML 片段沿用同一导出规则。CSS 值通过 token 解析检查字符串、数值和允许的函数；Typst 使用各自的长度、颜色和字体映射。

项目使用 Rust 1.98.1 及以上版本。CI 配置覆盖 Linux、Windows、macOS 的 stable 工具链，并单独检查 1.98.1；执行格式检查、Clippy、测试和 release 构建。测试包含本地网络服务、混合容器、Office 命名空间及确定性生成的畸形输入。

UEditor 的参考提交和许可证位置见 [规则来源](ueditor-reference.md)。

当前格式支持集中在语义标签和内联样式。外部样式表、动态脚本、复杂 CSS 和部分 Word 自动编号属于待支持项。布局表格沿用表格结构；选择 JSON 输出时，可以查看记录的 HTML 标签和属性。

### 编码

编码依次参考 BOM、HTTP charset 和 HTML meta，缺省使用 UTF-8。解码异常显示在终端日志中。

### 日志

`--verbose` 显示输入来源、字节数、选择器、输出目录和结果统计。错误信息包含处理阶段及相关 URL 或文件路径，例如：

```text
ue-weave: error: 正文选择器未找到：.missing。可使用 --selector content=CSS 指定正文容器
```

网页、图片和附件共用网络请求与重试逻辑。默认每次请求超时 30 秒，临时失败最多重试 2 次，间隔按 250ms、500ms、1s、2s、4s 递增。

超时、DNS 解析、连接故障、响应读取中断，以及 HTTP 408、429、500、502、503、504 会触发有限重试。HTTP 403、404 和 TLS 证书或协议错误直接返回。`Retry-After` 为 0–30 秒时按要求等待；更长等待或日期格式会结束当前请求，并输出原因。

```sh
ue-weave 'https://cse.seu.edu.cn/2026/1102/c22535a588376/page.htm' \
  --output output/cse --retries 3 --timeout 45 --verbose --json
```

`--retries 0` 关闭重试。日志写入 stderr，包含 URL、错误类型、底层原因、尝试次数和重试间隔。`--json` 的失败结果包含 `code` 与 `network` 对象，其中 `kind` 区分 `timeout`、`dns`、`tls`、`connect`、`http_status`、`read_response` 等，`attempts`、`http_status`、`cause` 提供详情。资源下载失败的信息通过日志和 `--json` 输出。

## 开发与测试

以下命令在项目根目录运行。查阅 UEditor 参考源码前，先获取子模块：

```sh
git submodule update --init
```

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo build --release
```

测试使用本地 fixture 和本地 HTTP 服务，覆盖格式识别、资源处理、CLI、输出预设和 IR 再导出。

真实样本覆盖教务处、计算机学院、电气学院和校史校友站点，包括纯图片文章、普通附件、内嵌 PDF、合并单元格、缺失元数据与导航栏标题干扰。样本来源见 [fixtures/SOURCES.md](../fixtures/SOURCES.md)。本地 HTTP 测试覆盖重试成功、次数耗尽、超时、TLS 错误、响应截断、资源大小限制和附件路径替换。

| 模块 | 职责 |
| --- | --- |
| `model.rs` | JSON IR 定义 |
| `parser.rs`、`inline.rs`、`table.rs` | DOM 到 IR |
| `rules.rs`、`css.rs`、`word.rs`、`inference.rs` | 格式识别与推断 |
| `profile.rs` | 输出预设和格式开关 |
| `site.rs`、`site_config.rs` | 模板识别与站点配置 |
| `network.rs` | 网络诊断与有限重试 |
| `renderer.rs`、`markdown.rs`、`typst.rs` | 文档导出 |
| `downloader.rs`、`regenerate.rs` | 资源下载和 IR 再导出 |
| `error.rs`、`log.rs` | 错误模型和日志 |

`vendor/ueditor` 保存 UEditor 源码参考副本。实现参考了 `_src/core/Editor.js`、`htmlparser.js`、`filternode.js`、`filterword.js`、`dtd.js`、`domUtils.js`、`_src/plugins/paste.js` 和 `_parse/parse.js`。规则说明见 [SOURCE_RULES.md](../SOURCE_RULES.md)。

### 真实页面验收

独立验收脚本从东南大学教务处“最新动态”列表收集文章，调用单页 CLI 生成样例：

```sh
node scripts/acceptance.mjs --output acceptance-output --max 200 --pages 20
```

脚本依赖 Node.js、curl 和 Cargo；安装 Typst 后会生成 PDF。默认下载图片，添加 `--download-attachments` 可下载附件。请求间隔为 350ms，默认最多抓取 200 篇文章、20 个列表页。

输出目录的 `index.html` 提供验收入口，`manifest.json` 汇总结果，`urls.txt` 保存文章 URL。各文章目录包含最终文档和运行日志。重复运行会重新抓取，可用不同输出目录保存各轮结果。
