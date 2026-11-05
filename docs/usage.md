# 详细使用说明

[返回首页](../README.md)

## 输出预设

`--profile` 接受 `conservative`、`balanced`、`aggressive`，也接受中文名称 `保守`、`均衡`、`激进`。

| 预设 | 输出效果 |
| --- | --- |
| `conservative`（默认） | 保留已识别的字体、字号、颜色、对齐和布局 |
| `balanced` | 统一字体和布局，保留颜色、对齐、缩进 |
| `aggressive` | 简化视觉格式，优先可读性和编辑便利性 |

预设作用于 HTML、Markdown 和 Typst。需要保留结构化正文时，可额外选择 JSON 输出。

### 格式开关

开关可以单独覆盖预设。省略值表示开启，`=false` 表示关闭。

| 开关 | 作用 | 保守 | 均衡 | 激进 |
| --- | --- | --- | --- | --- |
| `--strip-fonts` | 移除字体、字号 | 关 | 开 | 开 |
| `--strip-colors` | 移除文字颜色、背景色 | 关 | 关 | 开 |
| `--strip-layout` | 移除行距、段间距、固定表格尺寸、单元格内边距 | 关 | 开 | 开 |
| `--strip-alignment` | 移除显式文本对齐 | 关 | 关 | 开 |
| `--strip-indent` | 移除首行缩进声明，保留文本中的空格 | 关 | 关 | 开 |
| `--trim-leading-spaces` | 清理段落、标题和署名开头的普通空格、NBSP、全角空格及制表符 | 关 | 关 | 开 |
| `--drop-empty-paragraphs` | 移除纯空白段落 | 关 | 关 | 开 |
| `--native-markdown` | 简单列表和规则表格使用原生 Markdown，支持无表头表格 | 关 | 开 | 开 |
| `--strip-image-dimensions` | 移除图片显式宽高 | 关 | 关 | 关 |

无表头的规则表格会添加空表头，原有各行保持为数据行；使用 `--json` 可查看这项转换记录。单元格内换行使用 `<br>`，段落间使用 `<br><br>`。合并单元格、嵌套表格和混合表头使用按行排版的 HTML。表格语法遵循 [GFM 表格规范](https://github.github.com/gfm/#tables-extension-)。

字体、颜色和布局支持细分控制：

段首空白清理支持跨多个文本片段处理，保留段内空格和换行。含代码或显式预格式化样式的段落保留空白。使用 `--json` 可在终端查看清理前后的值及对应的 HTML 标签编号。使用 `--trim-leading-spaces=false` 可以关闭此项。

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
| `--input-ir <FILE>` | 从已有 JSON 正文重新导出 |
| `--formats <FORMATS>` | 选择 html、markdown、typst、json，多项用逗号分隔；默认前三项 |
| `--output <DIR>`、`-o <DIR>` | 指定输出目录 |
| `--selector <[FIELD=]CSS>` | 覆盖选择器，可重复；省略字段时指定正文 |
| `--site-config <FILE>` | 加载站点选择器 JSON 配置 |
| `--retries <N>` | 临时网络故障重试次数，默认 2，范围 0–5 |
| `--timeout <SECONDS>` | 单次请求总超时，默认 30 秒，范围 1–120 |
| `--download-images` | 下载正文图片 |
| `--download-attachments` | 下载附件并替换为本地链接 |
| `--asset-root <DIR>` | 从 JSON 导出时，指定图片和附件所在目录，包含 images/ 和 attachments/ |
| `--allow-private-network` | 允许网页与资源请求访问私网、回环和保留网段 |
| `--header <NAME=VALUE>` | 添加网页与资源请求的 HTTP header，可重复 |
| `--headers-file <FILE>` | 从 JSON 对象读取 HTTP header |
| `--include-metadata` | 在导出文档顶部显示文章元数据 |
| `--no-inference` | 关闭启发式推断 |
| `--json` | 以 JSON 输出运行结果摘要 |
| `--verbose` | 输出详细阶段日志 |
| `--color <MODE>` | 日志颜色：auto 自动（默认）、always 始终、never 关闭 |

header 文件格式为 JSON 对象，例如：

```json
{
  "Referer": "https://example.edu/",
  "X-Requested-With": "XMLHttpRequest"
}
```

命令行 header 与文件中的 header 合并；同名 header 后出现的值覆盖前面的值。不要设置 `Host`、`Content-Length`、`Connection` 或 `Transfer-Encoding`。header 值不会写入报告。

## 站点选择器配置

配置文件直接定义正文、标题、日期、来源、作者和发布者的 CSS 选择器，同一份配置适用于相同模板的网页和本地 HTML。项目提供 [configs/webplus.json](../configs/webplus.json) 和 [configs/article.json](../configs/article.json) 两份模板。

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

省略字段会使用内置规则；配置的元数据选择器匹配不到内容时，该字段留空。配置文件加载时检查 JSON 字段和 CSS 语法，错误信息标注文件路径及字段。内置学院模板使用 `.arti_*` 元数据，将文章标题与导航栏的 `.Article_Title` 分开提取。

## 编辑与重新导出

通过 `--formats json` 导出结构化正文后，可以修改 `reconstructed.json` 的 `blocks`、`runs` 等字段，再生成文档：

```sh
ue-weave --input-ir output/reconstructed.json \
  --output edited-output/ --profile balanced
```

输入 JSON 使用 `schema: {"name":"ue-weave-ir","version":2}` 标识格式。导入时检查版本、节点类型、标题级别及表格跨度，错误信息包含 JSON 字段路径。可选格式字段省略时使用缺省值。

重新导出时提供 JSON 和其中引用的本地图片、附件。

JSON 中的 `source_nodes` 对应 `original_nodes` 保存的网页节点，可据此查找原来的文字、属性和上下文。编辑正文时保留这些引用；新增内容可省略。日志和处理理由通过终端输出。

`--verbose` 左侧显示 HTML 上下文，右侧显示清洗后的文字，两侧标注位置对齐。终端不足 100 列时改为上下显示。

资源清单中的 `local_path` 使用 `images/文件名` 或 `attachments/文件名`。资源根目录默认为 JSON 所在目录，也可通过 `--asset-root edited/assets` 指定；此时资源位于 `edited/assets/images/` 和 `edited/assets/attachments/`。

普通正文编辑 `runs`，代码块编辑 `text`。标题级别写为 `"level": 2`，行距等格式字段填写具体值。

处理日志通过 `--verbose` 或 `--json` 查看。

终端错误使用 `error[E0001]` 格式，警告使用 `warning[W0001]` 格式。编号含义见[日志编号](diagnostics.md)。
