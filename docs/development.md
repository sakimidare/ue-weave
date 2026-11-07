# 开发说明

[返回首页](../README.md)

本文中的 IR 指程序内部及 JSON 输出使用的文章结构，DOM 指 HTML 解析后的节点树。

## 库接口与执行流程

`lib.rs` 提供 `Article::from_html`、`from_bytes`、`from_json`、`from_json_bytes`，以及 `to_html`、`to_markdown`、`to_typst`、`to_document`、`to_json` 等导出方法；`ParseOptions` 控制解析，`OutputOptions` 控制预设和格式开关，`Integrity` 给出正文与资源完整性检查结果。库错误通过 `Error::code()` 返回与 CLI 一致的编号。公开 API 使用中文文档，由 `missing_docs` 检查覆盖。

公开接口由 `tests/api.rs` 固化：所有构造器、访问器、导出方法、字段名和返回类型都以函数指针或类型断言写死，签名变化会直接导致该测试编译失败。修改公开 API 时需同步更新该文件并在提交信息中说明。`Metadata`、`Document`、`Attachment`、`Image` 和 `Profile` 标记了 `non_exhaustive`，新增字段或预设不视为破坏性变更；`OutputOptions` 保持可用结构体字面量构造，新增开关需要评估 semver。

`main.rs` 调用命令入口；`cli.rs` 管理 clap 参数和退出状态；`pipeline.rs` 依次读取输入、解析正文、处理资源、生成文件和汇报结果。URL 输入与本地文件通过 `Source` 枚举区分。一次处理按需创建一个网络客户端，网页与资源下载共用客户端及重试设置。

错误路径从原始参数读取颜色和 JSON 输出选项。解析在遇到错误参数时停止。

## 文章 JSON 与解析记录

`document.rs` 定义文章 JSON：文章信息、正文格式和资源。格式字段保存具体值，空值和默认布尔值省略。`model.rs` 定义解析数据，供日志、校验和格式处理使用。`ir_format.rs` 负责读取 JSON 并转换为渲染器使用的数据。

正文块、文字片段和资源通过 `source_nodes` 引用 `original_nodes` 中的节点。解析阶段使用 `scraper`/`ego-tree` 的真实 `NodeId` 建立临时映射，再输出稳定的 `n0`、`n1` 等快照 ID；不依赖 `NodeId` 的 Debug 字符串格式。节点记录包含 ID、父子关系、标签、属性和解码后的文本；节点 ID 在同一份文档中唯一。读取 JSON 时检查引用、父子关系和循环。用户新增的内容可以省略来源引用。

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

`--print-json-result` 的终端输出包含格式预设、开关和格式修改记录，以及推断、未知项和下载错误。终端结果中的 `content_integrity` 记录正文文字及顺序的检查结果。

`report.rs` 定义各输入方式及失败路径共用的运行报告。`warnings` 与 `notes` 按枚举级别分开；下载失败、正文或资源完整性不匹配产生 `partial`。报告字段和退出码见[JSON 运行报告](usage.md#json-运行报告)。

终端结果中的 `resource_integrity` 记录还原前后图片、超链接和 PDF 地址的数量是否一致。

### 资源下载

使用 `--allow-private-network` 请求校内网或本地地址。可使用重复的 `--header NAME=VALUE` 或 `--headers-file` 为网页、图片和附件请求提供站点要求的 header；header 值不写入报告。

网页和单个资源的读取上限为 64 MiB。图片按 JPEG、PNG、GIF、WebP 文件签名保存；图片响应类型与实际图片格式不同时，按实际格式保存并记录 W0015。HTML 提示页和其他非图片内容报告下载错误。

附件文件名依次参考响应头 `Content-Disposition`（包括 UTF-8 `filename*`）、附件标题和最终 URL。中文名称保留，路径分隔符转换为普通字符。ZIP 文件可保留 DOCX、XLSX、PPTX、EPUB 等原来的扩展名，OLE 文件可保留 DOC、XLS、PPT 等扩展名；记录中的 `detected_type` 表示检测到的容器类型。其他数据文件保留原来的扩展名，内容类型记为 `unknown`。内容签名与扩展名冲突时调整扩展名，并写入处理记录；程序、脚本或 HTML 等可执行内容使用 `.download`。文件名和内容类型均未提供扩展名时使用 `.bin`。资源以普通数据文件权限创建，同名文件使用新文件名。

下载器记录请求地址、最终地址、响应状态、内容类型、大小、SHA-256 和本地路径。文章 JSON 中的资源包含地址、名称和本地路径。目录和符号链接的实际位置按资源根目录校验。

### 样式与工程检查

`style_registry.rs` 统一记录通用格式的 IR 目标、表格和单元格属性集合，供解析、审计与 HTML 导出共用。Markdown 的 HTML 片段沿用同一导出规则。CSS 值通过 token 解析检查字符串、数值和允许的函数；Typst 使用各自的长度、颜色和字体映射。

项目使用 Rust 1.98.1 及以上版本。CI 配置覆盖 Linux、Windows、macOS 的 stable 工具链，并单独检查 1.98.1；执行格式检查、Clippy、测试和 release 构建。测试包含本地网络服务、混合容器、Office 命名空间及确定性生成的畸形输入。

`rust-toolchain.toml` 选择 stable，并安装 rustfmt 和 Clippy。release 使用 thin LTO 并剥离符号。CI 缓存 Rust 构建产物。贡献约定见 [CONTRIBUTING.md](../CONTRIBUTING.md)。

UEditor 的参考提交和许可证位置见 [规则来源](ueditor-reference.md)。

格式提取使用 HTML 标签和内联样式，布局表格按表格结构保存。

### 编码

编码依次参考 BOM、HTTP charset 和 HTML meta，缺省使用 UTF-8。解码异常显示在终端日志中。

### 日志

日志格式与编号见[日志编号](diagnostics.md)。错误和警告通过枚举建模，终端及 JSON 使用相同编号。

`log/filter.rs` 解析日志开关并决定显示级别，`diagnostic/topic.rs` 定义诊断所属类别。过滤顺序与示例见[日志开关](usage.md#日志开关)。快捷预设先加入全局规则，再加入显式 `--log` 规则；匹配时按编号、类别与严重度组合、类别、严重度、全局的优先级选择，同层后写覆盖。运行时提示及错误后的解释提示共用过滤配置，错误本身始终输出。进度行单独读取全局与 `progress` 设置。

`diagnostic/warning.rs` 的 `WarningKind` 管理警告和 W 编号，`diagnostic/note.rs` 的 `NoteKind` 管理提示和 N 编号。`DiagnosticKind` 组合两类枚举，`Diagnostic` 保存详情并统一序列化；等级由枚举类别确定。网络重试使用 N0028，具体网络错误码放在详情中。

`diagnostic/details.rs` 集中定义详情字段。布尔状态、属性名、错误编号和来源编号数组使用具体类型；原始值及转换结果可保存不同形态的 JSON 值。构造诊断时使用结构体字段，反序列化时检查未知字段和字段类型。

`thiserror` 定义错误消息，`anstyle` 定义颜色和强调样式，`anstream` 负责终端适配与颜色输出。`--color auto|always|never` 控制颜色；日志写入 stderr。

`--explain CODE` 的解释写入 stdout；首行编号和标题按错误、警告、提示分别使用红、黄、青色，正文保持普通颜色。`--explain` 可与 `--color` 组合使用。

`--verbose` 依次展示格式化 HTML 上下文、当前输出预设处理后的文本及提示，通过节点 ID 关联两段内容。HTML 保留标签、属性和实体，标注相关属性值或节点；输出文本标注对应文字，无文字输出的节点单独提示。`log/context.rs` 生成 HTML 片段，`log/output_context.rs` 索引输出文字，`log/render.rs` 使用 `annotate-snippets` 渲染，并通过 `line_numbering(false)` 关闭行号。上下文索引仅在详细日志开启时建立。

`log/layout.rs` 使用 `comfy-table` 将 HTML 与输出文字左右排列，对齐两侧标注行；库负责 ANSI 样式和 Unicode 显示宽度。`terminal_size` 读取 stderr 所在终端的宽度，少于 100 列时上下排列，无法读取宽度时使用 120 列。

`--verbose` 显示输入来源、字节数、选择器、输出目录和结果统计。错误信息包含处理阶段及相关 URL 或文件路径，例如：

```text
error[E0007]: 正文选择器未找到：.missing。可使用 --selector content=CSS 指定正文容器
```

网页、图片和附件共用网络请求与重试逻辑。默认每次请求超时 30 秒，临时失败最多重试 2 次，间隔按 250ms、500ms、1s、2s、4s 递增。

DNS 解析器用 `RoutingError` 包装地址解析及网段检查错误。请求分类检查这些类型、`rustls::Error`、`reqwest::Error` 和 `io::ErrorKind`；未知错误采用请求阶段提供的分类。读取大小上限由 `network::MAX_BODY_SIZE` 统一定义。

超时、DNS 解析、连接故障、响应读取中断，以及 HTTP 408、429、500、502、503、504 会触发有限重试。HTTP 403、404 和 TLS 证书或协议错误直接返回。`Retry-After` 为 0–30 秒时按要求等待；更长等待或日期格式会结束当前请求，并输出原因。

```sh
ue-weave 'https://cse.seu.edu.cn/2026/1102/c22535a588376/page.htm' \
  --output output/cse --retries 3 --timeout 45 --verbose --print-json-result
```

`--retries 0` 关闭重试。日志写入 stderr，包含 URL、错误类型、底层原因、尝试次数和重试间隔。`--print-json-result` 的失败结果包含 `code` 与 `network` 对象，其中 `kind` 区分 `timeout`、`dns`、`tls`、`connect`、`http_status`、`read_response` 等，`attempts`、`http_status`、`cause` 提供详情。资源下载失败的信息通过日志和 `--print-json-result` 输出。

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
| `lib.rs`、`cli.rs`、`pipeline.rs` | 库接口、命令行参数和执行流程 |
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

### UEditor 内容规则

导出器通过 `output_runs.rs` 合并相邻且最终格式相同的普通文字；链接、图片、换行、行内代码保留边界。合并在输出预设生效后进行，解析结果及文字来源记录保持原样。Markdown 对相邻强调标记、首尾空白和标点边界使用 HTML 标签；测试使用 `pulldown-cmark` 核对生成内容的文字和格式。

`code.rs` 读取 pre 的 `brush:语言`、`language-语言`、`data-lang`，也读取直接子 code 的语言标记。`code_language` 写入文章 JSON，HTML 使用 `language-*` class，Markdown 使用围栏语言，Typst 使用 raw 的 lang 参数。代码中的 br 转为换行；带 brush 标记的 pre 将 NBSP 转为普通空格（N0032），其他 pre 保留 NBSP。代码字面文本保留 `undefinedundefined`。

文字片段的 `inline_kind` 可为 `code`、`keyboard`、`sample`、`variable`。定义列表使用 `definition_list`，其 children 按原顺序保存 `definition_term` 和 `definition_description`；同一术语可有多个解释，也可保留多个术语共用解释的顺序。

`tag_registry.rs` 为解析和诊断提供块、行内、容器、部分转换、排除和未知标签的分类。colgroup/col 和已支持的容器不报告 W0009；figure/details 等展开为内容时保留该警告。媒体地址经过 URL 检查后转换为链接，SVG 使用 title/desc/text，MathML 优先使用 annotation。静态 HTML 解析关闭 scripting 模式，以读取 noscript 的备用内容。

`depth.rs` 在正文递归解析前迭代检查 HTML 深度，上限为 256；正文块嵌套上限为 24，以使含表格的 JSON 可重新读入。超限部分迭代提取文本并报告 W0019。`_baidu_bookmark_*` span 移除并报告 N0031。

本地样例为 `fixtures/ueditor-semantics.html`；`semantics_tests.rs` 检查导出与 JSON 再读入、媒体资源、文本顺序和深层 HTML。

### 真实页面验收命令

独立验收脚本从东南大学教务处“最新动态”列表收集文章，调用单页 CLI 生成样例：

```sh
node scripts/acceptance.mjs --output acceptance-output --max 200 --pages 20
```

验收脚本支持传递 `--allow-private-network`：

```sh
node scripts/acceptance.mjs --allow-private-network
```

脚本依赖 Node.js、curl 和 Cargo；安装 Typst 后会生成 PDF。默认下载图片，添加 `--download-attachments` 可下载附件。请求间隔为 350ms，默认最多抓取 200 篇文章、20 个列表页。

输出目录的 `index.html` 提供验收入口，`manifest.json` 汇总结果，`urls.txt` 保存文章 URL。各文章目录包含最终文档和运行日志。重复运行会重新抓取，可用不同输出目录保存各轮结果。

抓取教务处 2020 年及之后的公开文章：

```sh
cargo build --locked
node scripts/jwc-archive.mjs output/jwc-aggressive-2020
```

此脚本从官网首页遍历可达栏目及全部分页，按文章 URL 年份筛选、文章 ID 去重，使用激进模式和顶部元数据。下载图片，附件保留链接，输出按年份分目录；安装 Typst 后生成 PDF。列表最多并发 4 个请求，文章最多并发 3 个，每批间隔 300ms。`state.json` 保存进度，重复运行同一输出目录可继续未完成任务。栏目请求失败写入 `manifest.json` 的 `discovery_errors`。

完成抓取后，运行 `node scripts/check-jwc-archive.mjs 输出目录` 可复核失败页面，区分校内访问、原网页正文为空等情况，并生成 `quality-summary.json`。再次运行抓取脚本会用这些记录更新验收首页。

增量复核可使用 `--retry-failed` 重试失败文章，`--retry-images` 重试图片下载失败的文章，或 `--refresh --ids 318274,346303` 重新生成指定文章。`--ids` 使用文章网址中的文章编号。校园网连接可通过 `--resolve jwc.seu.edu.cn=10.64.84.122 --allow-private-network` 指定访问地址；地址按当前网络配置填写。抓取脚本会将这些网络选项传给单页工具。
