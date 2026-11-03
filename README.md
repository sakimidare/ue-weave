# ue-weave

用 Rust 编写的 UEditor / WebPlus 文章结构还原工具。读取网页或本地 HTML，提取正文，生成可编辑的 JSON 中间表示（IR），并导出 HTML、Markdown 和 Typst。

支持段落、标题、文本格式、图片、附件、链接、列表和表格，提供保守、均衡、激进三档输出预设及独立格式开关。

## 安装

在项目目录执行：

```sh
git submodule update --init
cargo install --path .
```

也可以直接构建运行：

```sh
cargo build --release
./target/release/ue-weave --help
```

## 快速开始

抓取文章：

```sh
ue-weave 'https://jwc.seu.edu.cn/2026/0915/c21679a583204/page.htm' \
  --output output/ --profile aggressive
```

读取本地 HTML：

```sh
ue-weave --input page.html --output output/
```

指定正文容器并下载资源：

```sh
ue-weave 'https://example.org/article.html' \
  --selector '.article-body' --output output/ \
  --download-images --download-attachments
```

默认使用 `--selector auto`，依次识别 `.Article_Content`、`.wp_articlecontent`，适用于教务处、计算机学院、校史校友等 WebPlus 模板。显式指定 `--selector` 时使用指定的容器。

元数据支持 `.Article_Title`、`.Article_PublishDate`、`.Article_Source`、`.Article_Author`，以及 `.arti_title`、`.arti_update`、`.arti_source`、`.arti_author`。`.arti_publisher` 单独记录为发布者 `publisher`，日期等字段会清理“发布时间：”等标签前缀。报告记录请求的选择器和实际使用的选择器。

## 输出文件

| 文件 | 内容 |
| --- | --- |
| `source.bin` | 完整输入字节 |
| `original.html` | 原始正文快照 |
| `reconstructed.json` | JSON IR，包含元数据、正文、资源和原始节点 |
| `reconstructed.html` | 语义化 HTML |
| `reconstructed.md` | Markdown 文档 |
| `reconstructed.typ` | Typst 文档 |
| `report.json` | 还原记录、格式变更、下载结果和不确定项 |
| `source.html` | 使用 `--keep-raw` 时保存的完整解码页面 |
| `images/`、`attachments/` | 下载的图片和附件 |

网页中的相对链接按文章 URL 转为绝对 URL。成功下载的资源使用本地相对路径，方便整体移动输出目录。本地 HTML 输入中的路径保持原样。

安装 Typst 后可以生成 PDF：

```sh
typst compile output/reconstructed.typ output/result.pdf
```

### 显示文章元数据

使用 `--include-metadata` 在 HTML、Markdown、Typst 正文上方添加标题、日期、作者、来源和发布者：

```sh
ue-weave --input page.html --output output/ --include-metadata
ue-weave --input-ir output/reconstructed.json --output edited-output/ --include-metadata
```

标题使用一级标题，其余字段逐行显示；缺失或空白字段自动跳过。三档预设默认关闭此选项，也可显式设置 `--include-metadata=false`。元数据取自 IR 的 `metadata`，可结合选择器配置提取，或编辑 JSON 后重新导出。JSON 正文结构和原始快照保持原样。

## 输出预设

`--profile` 接受 `conservative`、`balanced`、`aggressive`，也接受中文名称 `保守`、`均衡`、`激进`。

| 预设 | 输出效果 |
| --- | --- |
| `conservative`（默认） | 保留已识别的字体、字号、颜色、对齐和布局 |
| `balanced` | 统一字体和布局，保留颜色、对齐、缩进 |
| `aggressive` | 简化视觉格式，优先可读性和编辑便利性 |

预设作用于 HTML、Markdown 和 Typst。完整 IR 和原始快照保留，便于重新选择输出样式。

### 格式开关

开关可以单独覆盖预设。省略值表示开启，`=false` 表示关闭。

| 开关 | 作用 | 保守 | 均衡 | 激进 |
| --- | --- | --- | --- | --- |
| `--strip-fonts` | 移除字体、字号 | 关 | 开 | 开 |
| `--strip-colors` | 移除文字颜色、背景色 | 关 | 关 | 开 |
| `--strip-layout` | 移除行距、段间距、固定表格尺寸、单元格内边距 | 关 | 开 | 开 |
| `--strip-alignment` | 移除显式文本对齐 | 关 | 关 | 开 |
| `--strip-indent` | 移除首行缩进声明，保留文本中的空格 | 关 | 关 | 开 |
| `--drop-empty-paragraphs` | 移除纯空白段落 | 关 | 关 | 开 |
| `--native-markdown` | 简单列表和带表头的简单表格使用原生 Markdown | 关 | 关 | 开 |
| `--strip-image-dimensions` | 移除图片显式宽高 | 关 | 关 | 关 |

字体、颜色和布局支持细分控制：

| 组合开关 | 细分开关 |
| --- | --- |
| `--strip-fonts` | `--strip-font-family`、`--strip-font-size` |
| `--strip-colors` | `--strip-text-color`、`--strip-background-color` |
| `--strip-layout` | `--strip-line-height`、`--strip-paragraph-spacing`、`--strip-table-dimensions`、`--strip-cell-padding` |

优先级为 **细分开关 > 组合开关 > profile**，与参数顺序无关。

```sh
# 激进档保留文字颜色
ue-weave --input page.html --output output/ \
  --profile aggressive --strip-text-color=false

# 均衡档保留字号
ue-weave --input page.html --output output/ \
  --profile balanced --strip-font-size=false

# 保守档移除字体名称和图片固定尺寸
ue-weave --input page.html --output output/ \
  --profile conservative --strip-font-family --strip-image-dimensions
```

三档均保留正文、粗体、斜体、下划线、删除线、正文内换行、链接、图片、列表层级和表格合并结构。Markdown 通过内嵌 HTML 表达复杂表格、特殊编号和样式；Typst 将结构映射为排版元素，字体由本地字体库解析，缺失时使用回退字体。

## 常用参数

| 参数 | 用途 |
| --- | --- |
| `--input <FILE>` | 读取本地 HTML |
| `--input-ir <FILE>` | 从已有 JSON IR 重新导出 |
| `--output <DIR>`、`-o <DIR>` | 指定输出目录 |
| `--selector <[FIELD=]CSS>` | 覆盖选择器，可重复；省略字段时指定正文 |
| `--site-config <FILE>` | 加载站点选择器 JSON 配置 |
| `--retries <N>` | 临时网络故障重试次数，默认 2，范围 0–5 |
| `--timeout <SECONDS>` | 单次请求总超时，默认 30 秒，范围 1–120 |
| `--download-images` | 下载正文图片 |
| `--download-attachments` | 下载附件并替换为本地链接 |
| `--keep-raw` | 额外保存完整解码页面 |
| `--include-metadata` | 在导出文档顶部显示文章元数据 |
| `--no-inference` | 关闭启发式推断 |
| `--json` | 以 JSON 输出运行结果摘要 |
| `--verbose` | 输出详细阶段日志 |

## 站点选择器配置

配置文件直接定义正文、标题、日期、来源、作者和发布者的 CSS 选择器，同一份配置适用于相同模板的网页和本地 HTML。项目提供 [configs/webplus.json](configs/webplus.json) 和 [configs/article.json](configs/article.json) 两份模板。

```json
{
  "content": ".article-body",
  "title": "h1.article-title",
  "date": ".publish-date",
  "source": ".article-source",
  "author": ".article-author",
  "publisher": ".publisher"
}
```

```sh
ue-weave 'https://cse.seu.edu.cn/2026/1102/c22535a588376/page.htm' \
  --site-config configs/webplus.json --output output/cse

ue-weave --input fixtures/cse-588376.html \
  --site-config configs/webplus.json \
  --output output/local
```

各字段逐项覆盖，优先级为：命令行 `--selector` > 配置文件 > 内置模板规则。`content: "auto"` 使用自动正文识别。

`--selector` 支持 `content`、`title`、`date`、`source`、`author`、`publisher` 六个字段，可以重复传入。同一字段最后一次设置生效：

```sh
ue-weave --input page.html --output output/ \
  --site-config configs/webplus.json \
  --selector 'content=.article-body' \
  --selector 'title=h1' \
  --selector 'date=.publish-date' \
  --selector 'author=.byline'
```

`--selector '.article-body'` 等价于 `--selector 'content=.article-body'`。省略配置文件时也可直接传入这些字段；`--selector content=auto` 启用正文自动识别。CSS 属性选择器中的等号照常使用，例如 `--selector 'title=h1[data-role="title"]'`。

省略字段会使用内置规则；配置的元数据选择器匹配不到内容时，该字段留空。配置文件加载时检查 JSON 字段和 CSS 语法，错误信息标注文件路径及字段。报告中的 `site_selectors` 记录生效配置。内置学院模板使用 `.arti_*` 元数据，将文章标题与导航栏的 `.Article_Title` 分开提取。

## 编辑与重新导出

修改 `reconstructed.json` 的 `blocks`、`runs` 等字段后，重新生成文档：

```sh
ue-weave --input-ir output/reconstructed.json \
  --output edited-output/ --profile balanced
```

输入 JSON 同目录需包含 `original.html`。再导出会复制原始快照、已有源文件，以及资源清单中 `local_path` 指定的本地图片和附件。资源从输入包的 `images/`、`attachments/` 读取。

普通正文以 `runs` 为编辑入口，代码块以 `text` 为编辑入口。再导出沿用来源报告中的还原记录。

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

`report.json` 的 `output_profile`、`output_options`、`output_changes` 分别记录预设、最终开关和实际格式变更。`warnings` 汇总推断与未知项，`attachment_errors` 记录附件下载失败信息。`metadata.content_integrity` 检查清理后的非空白正文文字及顺序。

当前格式支持集中在语义标签和内联样式。外部样式表、动态脚本、复杂 CSS 和部分 Word 自动编号属于待支持项。布局表格沿用表格结构；原始格式可通过正文快照和原始节点查阅。

### 编码与快照

编码依次参考 BOM、HTTP charset 和 HTML meta，缺省使用 UTF-8。编码识别和解码异常记录在报告中。

`original.html` 优先使用经 DOM 校验的正文源码片段，保留实体写法、属性引号和注释。定位存在歧义时使用 DOM 序列化，并记录 `raw-snapshot-fallback`。`source.bin` 保存输入的完整字节，在正文解析失败时也可查阅。

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

`--retries 0` 关闭重试。日志写入 stderr，包含 URL、错误类型、底层原因、尝试次数和重试间隔。`--json` 的失败结果包含 `code` 与 `network` 对象，其中 `kind` 区分 `timeout`、`dns`、`tls`、`connect`、`http_status`、`read_response` 等，`attempts`、`http_status`、`cause` 提供详情。资源下载失败的信息也记录在报告中。

## 开发与测试

```sh
cargo fmt --check
cargo test
```

测试使用本地 fixture 和本地 HTTP 服务，覆盖格式识别、资源处理、CLI、输出预设和 IR 再导出。

真实样本覆盖教务处、计算机学院、电气学院和校史校友站点，包括纯图片文章、普通附件、内嵌 PDF、合并单元格、缺失元数据与导航栏标题干扰。样本来源见 [fixtures/SOURCES.md](fixtures/SOURCES.md)。本地 HTTP 测试覆盖重试成功、次数耗尽、超时、TLS 错误、响应截断、资源大小限制和附件路径替换。

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

`vendor/ueditor` 保存 UEditor 源码参考副本。实现参考了 `_src/core/Editor.js`、`htmlparser.js`、`filternode.js`、`filterword.js`、`dtd.js`、`domUtils.js`、`_src/plugins/paste.js` 和 `_parse/parse.js`。规则说明见 [SOURCE_RULES.md](SOURCE_RULES.md)。

### 真实页面验收

独立验收脚本从东南大学教务处“最新动态”列表收集文章，调用单页 CLI 生成样例：

```sh
node scripts/acceptance.mjs --output acceptance-output --max 200 --pages 20
```

脚本依赖 Node.js、curl 和 Cargo；安装 Typst 后会生成 PDF。默认下载图片，添加 `--download-attachments` 可下载附件。请求间隔为 350ms，默认最多抓取 200 篇文章、20 个列表页。

输出目录的 `index.html` 提供验收入口，`manifest.json` 汇总结果，`urls.txt` 保存文章 URL。各文章目录包含原始快照、重建文件和运行日志。重复运行会重新抓取，可用不同输出目录保存各轮结果。
