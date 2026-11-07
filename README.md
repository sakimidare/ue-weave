# ue-weave

把网页文章整理成 HTML、Markdown 或 Typst，方便阅读、编辑和排版。支持 UEditor / WebPlus 网站中的标题、段落、图片、附件、链接、列表和表格。

代码块保留语言、换行和缩进；支持行内代码、术语解释列表及备用图片。视频、音频和嵌入页面转换为链接，图形和公式保留可提取的文字并给出提示。

## 安装

安装 Rust 1.98.1 或更新版本后，在项目目录运行：

```sh
cargo install --path .
```

## 导出一篇文章

```sh
ue-weave 'https://jwc.seu.edu.cn/2026/0915/c21679a583204/page.htm' \
  --output output/
```

也可以读取已保存的网页：

```sh
ue-weave --input page.html --output output/
```

每次处理一篇文章。默认自动查找正文，并生成：

| 文件 | 用途 |
| --- | --- |
| `reconstructed.html` | 网页正文 |
| `reconstructed.md` | 用 Markdown 编辑器打开 |
| `reconstructed.typ` | 用 Typst 排版、生成 PDF |

只需要一种格式时：

```sh
ue-weave --input page.html --output output/ --formats markdown
```

多种格式用逗号分隔，例如 `--formats html,typst`。重复使用同一输出目录时，同名文档会更新，其他已有文件保留。

字体名中的编辑器异常字符串默认会自动修复；如需保留原始值，使用 `--no-repair-corrupt-fonts`。

## 选择输出风格

| 选项 | 效果 |
| --- | --- |
| `--profile conservative`（默认） | 尽量保留已识别的字体、字号、颜色、对齐和间距 |
| `--profile balanced` | 简化字体和间距，保留颜色、对齐和缩进 |
| `--profile aggressive` | 清理字体、颜色、缩进和空段落，方便阅读和编辑 |

例如，导出适合继续编辑的 Markdown：

```sh
ue-weave --input page.html --output output/ \
  --formats markdown --profile aggressive
```

可以单独调整某项效果。激进档保留文字颜色时，加上 `--strip-text-color=false`。完整选项见[格式设置](docs/usage.md#输出预设)。

## 下载图片和附件

```sh
ue-weave 'https://jwc.seu.edu.cn/2026/0915/c21679a583204/page.htm' \
  --output output/ --download-images --download-attachments
```

图片保存在 `images/`，附件保存在 `attachments/`，文档中的对应链接会指向下载的文件。移动文章时，将整个输出目录一起移动。

默认保留远程资源链接。网页中的相对链接会转为完整网址；本地 HTML 中的路径保持原样。

部分学校会要求特定的 HTTP header。可重复使用 `--header NAME=VALUE`；这些 header 用于与输入网址协议、主机和端口相同的页面、图片及附件请求：

```sh
ue-weave 'https://example.edu/article.htm' \
  --header 'Referer=https://example.edu/' \
  --header 'X-Requested-With=XMLHttpRequest' \
  --output output/
```

也可以从 JSON 对象文件读取：

```json
{
  "Referer": "https://example.edu/",
  "X-Requested-With": "XMLHttpRequest"
}
```

```sh
ue-weave 'https://example.edu/article.htm' \
  --headers-file headers.json --output output/
```

header 值不会写入报告。

## 显示标题、日期和作者

加上 `--include-metadata`，将网页中找到的标题、日期、作者等信息放到正文上方：

```sh
ue-weave --input page.html --output output/ --include-metadata
```

## 指定正文位置

自动查找未找到正文时，可用网页中的标签或类名指定位置：

```sh
ue-weave --input page.html --output output/ \
  --selector '.article-body' --selector 'title=h1'
```

这里的 `.article-body` 表示正文所在的 HTML 元素类名，`h1` 表示标题标签。日期、作者等字段也可以分别指定，详见[网页配置](docs/usage.md#站点选择器配置)。

## 生成 PDF

安装 Typst 后执行：

```sh
typst compile output/reconstructed.typ output/result.pdf
```

## 更多选项

- `--verbose`：查看详细处理过程。
- `--quiet`：只显示错误；`--log` 可按类别或编号调整日志，覆盖快捷预设。完整优先级和组合示例见[日志开关](docs/usage.md#日志开关)。
- `--explain N0002`：查看编号含义、具体例子和处理方法，可单独运行。
- `--timeout 45`：将每次请求的超时设为 45 秒，默认 30 秒。
- `--retries 3`：临时网络故障重试 3 次，默认 2 次。
- `--allow-private-network`：访问内网网站时使用。
- `--formats ir`：导出 JSON 正文，供程序读取或编辑。
- `--print-json-result`：在终端输出 JSON 格式的运行结果。
- `--help`：查看全部参数。

详细说明见[使用手册](docs/usage.md)。实现方式、测试和验收脚本见[开发说明](docs/development.md)。

## 作为库使用

项目同时是 Rust 库：解析已抓取或已保存的 HTML，直接生成文档。

```rust
use ue_weave::{Article, ParseOptions, OutputOptions, Profile};

let article = Article::from_html(html, ParseOptions::new())?;
let document = article.to_document_with(OutputOptions::new(Profile::Aggressive));

println!("{}", document.markdown);
```

- `ParseOptions` / `SiteSelectors`：正文与字段选择器、原网页地址、是否推断；
- `Article::from_bytes`：自动识别 BOM 与 `<meta charset>`；
- `Article::from_json` / `from_json_bytes`：读取 `--formats ir` 导出的正文，对应 `to_json` 与 `to_json_bytes`；
- `to_json_compact`：省略来源节点，体积约为完整 JSON 的四分之一，仍可被 `from_json` 读取；
- `metadata()`、`integrity()`、`diagnostics()`、`attachments()`、`images()`：结构化读取；
- `OutputOptions`：预设、元数据头，以及全部 `strip-*` 细分开关。

完整接口见 `cargo doc --open`。
