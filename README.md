# ue-recover

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
./target/release/ue-recover --help
```

## 快速开始

抓取文章：

```sh
ue-recover 'https://jwc.seu.edu.cn/2026/0915/c21679a583204/page.htm' \
  --output output/ --profile aggressive
```

读取本地 HTML：

```sh
ue-recover --input page.html --output output/
```

指定正文容器并下载资源：

```sh
ue-recover 'https://example.org/article.html' \
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

## 输出预设

`--profile` 接受 `conservative`、`balanced`、`aggressive`，也接受中文名称 `保守`、`均衡`、`激进`。

| 预设 | 输出效果 |
| --- | --- |
| `conservative`（默认） | 尽量保留已识别的字体、字号、颜色、对齐和布局 |
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
ue-recover --input page.html --output output/ \
  --profile aggressive --strip-text-color=false

# 均衡档保留字号
ue-recover --input page.html --output output/ \
  --profile balanced --strip-font-size=false

# 保守档移除字体名称和图片固定尺寸
ue-recover --input page.html --output output/ \
  --profile conservative --strip-font-family --strip-image-dimensions
```

三档均保留正文、粗体、斜体、下划线、删除线、正文内换行、链接、图片、列表层级和表格合并结构。Markdown 通过内嵌 HTML 表达复杂表格、特殊编号和样式；Typst 将结构映射为排版元素，字体由本地字体库解析，缺失时使用回退字体。

## 常用参数

| 参数 | 用途 |
| --- | --- |
| `--input <FILE>` | 读取本地 HTML |
| `--input-ir <FILE>` | 从已有 JSON IR 重新导出 |
| `--output <DIR>`、`-o <DIR>` | 指定输出目录 |
| `--selector <CSS>` | 指定正文选择器 |
| `--download-images` | 下载正文图片 |
| `--download-attachments` | 下载附件并替换为本地链接 |
| `--keep-raw` | 额外保存完整解码页面 |
| `--no-inference` | 关闭启发式推断 |
| `--json` | 以 JSON 输出运行结果摘要 |
| `--verbose` | 输出详细阶段日志 |

## 编辑与重新导出

修改 `reconstructed.json` 的 `blocks`、`runs` 等字段后，重新生成文档：

```sh
ue-recover --input-ir output/reconstructed.json \
  --output edited-output/ --profile balanced
```

输入 JSON 同目录需包含 `original.html`。再导出会复制原始快照、已有源文件，以及资源清单中 `local_path` 指定的本地图片和附件。资源从输入包的 `images/`、`attachments/` 读取。

普通正文以 `runs` 为编辑入口，代码块以 `text` 为编辑入口。再导出沿用来源报告中的还原记录。

## 还原规则与报告

解析流程基于静态 HTML 节点和属性。格式来源包括语义标签、内联样式、UEditor 标记和 Word 粘贴残留。

- 提取段落、换行、列表、表格及文本格式。
- 识别 `sudyfile-attr`、`isupload` 和附件链接，清理附件图标。
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
ue-recover: error: 正文选择器未找到：.missing。请确认页面是文章详情页，或使用 --selector 指定正文容器
```

网络请求配置了 30 秒超时、User-Agent 和 HTTP 状态检查。

## 开发与测试

```sh
cargo fmt --check
cargo test
```

测试使用本地 fixture 和本地 HTTP 服务，覆盖格式识别、资源处理、CLI、输出预设和 IR 再导出。

| 模块 | 职责 |
| --- | --- |
| `model.rs` | JSON IR 定义 |
| `parser.rs`、`inline.rs`、`table.rs` | DOM 到 IR |
| `rules.rs`、`css.rs`、`word.rs`、`inference.rs` | 格式识别与推断 |
| `profile.rs` | 输出预设和格式开关 |
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
