# 详细使用说明

[返回首页](../README.md)

保存的网页可指定原网址，以补全图片、附件和链接：

```sh
ue-weave --input page.html --source-url https://jwc.seu.edu.cn/2024/0108/c21681a478415/page.htm --output output/
```

校园网络解析不稳定时，可固定已确认的连接地址。HTTPS 仍使用原域名校验证书：

```sh
ue-weave URL --resolve jwc.seu.edu.cn=10.64.84.122 --allow-private-network --output output/
```

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
| `--no-repair-corrupt-fonts` | 不修复字体名中的编辑器异常字符串 | 关 | 关 | 关 |
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

# 保留损坏字体名的原始值
ue-weave --input page.html --output output/ \
  --no-repair-corrupt-fonts
```

字体名中由编辑器产生的 `undefined` 包装默认会被清理，并对包含空格的字体名补充一对引号。例如 `undefinedundefinedmicrosoft yaheiundefinedundefined` 会还原为 `"microsoft yahei"`。添加 `--json` 可在标准输出的 `output_changes` 中查看修复记录；使用 `--no-repair-corrupt-fonts` 可保留原值。移除字体的开关仍会按所选预设生效。

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
| `--verbose` | 输出详细阶段日志；等价于 `--log all=context` |
| `--quiet` | 只输出错误；等价于 `--log all=off` |
| `--log <[TOPIC=]LEVEL>` | 日志开关，可重复或用逗号分隔 |
| `--color <MODE>` | 日志颜色：auto 自动（默认）、always 始终、never 关闭 |

header 文件格式为 JSON 对象，例如：

```json
{
  "Referer": "https://example.edu/",
  "X-Requested-With": "XMLHttpRequest"
}
```

命令行 header 与文件中的 header 合并；同名 header 后出现的值覆盖前面的值。自定义 header 只发送到与输入网址协议、主机和端口相同的地址，每次重定向也会检查。使用本地 HTML 输入时，自定义 header 不发送。header 值不会写入报告。

## 日志开关

`--log` 接受 `[topic[.severity]=]level`，可重复，也可用逗号分隔：

```sh
ue-weave URL -o out --log image=list --log attachment=list
ue-weave URL -o out --log style.warning=list
ue-weave URL -o out --log style.note=off,network=list
ue-weave URL -o out --log W0014=list
```

领域描述“问题出在网页的哪一部分”，级别描述“输出多细”，两者可以单独使用，也可以用 `领域.严重度` 组合。

级别：

| 级别 | 输出 |
| --- | --- |
| `off` | 不输出 |
| `summary` | 按编号聚合计数（warning 默认） |
| `list` | 逐条显示详情（地址、属性等），不含 HTML 上下文 |
| `context` | 每条附 HTML 片段与输出文字对照（`--verbose` 的效果） |

领域：

| 领域 | 覆盖 |
| --- | --- |
| `progress` | Reading / Fetching / Parsing / Writing / Finished |
| `network` | 网络重试 |
| `encoding` | 编码识别与解码替换 |
| `image` | 图片下载、图片尺寸、VML 图片 |
| `attachment` | 附件下载、附件属性、附件图标、附件标题补全、内嵌 PDF |
| `style` | 样式过滤、映射、跳过、表格与列表样式 |
| `structure` | 空元素、空段落、排除元素、嵌套限制、文档字段 |
| `integrity` | 正文与资源完整性 |
| `inference` | 标题、署名推断 |
| `word` | Word 与编辑器残留 |

额外的选择器：

| 写法 | 含义 |
| --- | --- |
| `all` | 全部诊断与进度行 |
| `warning`、`note` | 按严重度选择全部领域 |
| `W0014`、`N0037` | 单个编号 |
| `list`（省略类别） | 等价于 `all=list` |

优先级从高到低：

| 优先级 | 设置 | 示例 |
| --- | --- | --- |
| 1 | 具体编号 | `W0014=off` |
| 2 | 类别 + 警告或提示 | `style.warning=list` |
| 3 | 类别 | `style=list` |
| 4 | 警告或提示 | `warning=summary`、`note=off` |
| 5 | 全局设置 | `all=off`，简写为 `off` |
| 6 | 快捷预设 | `--verbose`、`--quiet` |
| 7 | 默认设置 | 见下文 |

每条诊断采用匹配到的最高优先级设置。同一优先级按命令行从左到右处理，后写的覆盖先写的；逗号分隔的设置也遵循此规则。高优先级设置始终有效，即使低优先级设置写在后面。

编号不区分大小写，`w0001=summary` 与 `W0001=summary` 相同。设置某个编号只调整该编号，其他诊断沿用已有设置。例如，`--log w0001=summary` 设置编码识别警告的显示方式，样式警告 W0014 仍按默认方式汇总。只想显示 W0001 时使用 `--log off,w0001=summary`；本次没有 W0001 时就不显示该项。

编号必须存在，例如 `W9999=off` 会返回 E0017。错误始终显示，`E0001=off` 等错误编号设置也会返回 E0017。

默认级别：warning 为 `summary`；带地址或位置的编号（`W0002`、`W0003`、`W0004`、`W0005`、`W0006`、`W0015`）为 `list`；其余 note 为 `off`。运行时提示 `N0028`（重试）、`N0029`（查看详情）、`N0030`（解释提示）默认显示，可用 `note=off`、`N0030=off` 或 `--quiet` 关闭。进度行默认关闭。错误始终输出，不受日志开关影响。

`--quiet` 将全局预设为 `off`，`--verbose` 将全局预设为 `context`。显式的 `--log` 设置覆盖快捷预设，与这两个参数的位置无关。`--quiet` 和 `--verbose` 互斥，同时使用会报错。

组合示例（错误始终显示）：

```sh
# 关闭进度、警告和提示；交换这两个参数的位置，结果相同。
ue-weave URL -o out --verbose --log off

# 只显示图片相关诊断及其 HTML 上下文。
ue-weave URL -o out --verbose --log off --log image=context

# 显示样式相关诊断，关闭其中的 W0014。
ue-weave URL -o out --verbose --log off --log style=list --log W0014=off

# 同一类别后写的生效：样式诊断关闭。
ue-weave URL -o out --log style=list,style=off

# 编号优先于类别：W0014 仍逐条显示。
ue-weave URL -o out --log W0014=list,style=off

# 只显示提示。
ue-weave URL -o out --quiet --log note=list
```

进度行由全局设置或 `progress` 控制，`warning`、`note` 和具体诊断编号不控制进度行。`progress=off` 和 `progress=summary` 均关闭进度行；`progress=list` 和 `progress=context` 显示进度行。

`--log all=summary`（简写为 `--log summary`）也会隐藏进度行。需要同时显示进度时，使用 `--log all=summary,progress=list`；类别设置 `progress=list` 优先于全局设置。`--log progress=off` 可与上下文日志一起使用，HTML 与输出文字的排版宽度保持一致。

日志开关只影响终端日志，不改变 `--json` 结果和退出码。

## JSON 运行报告

`--json` 将运行报告写入标准输出，日志写入标准错误。可分别保存：

```sh
ue-weave URL -o output --json --log style=list > result.json 2> run.log
```

网页、本地 HTML、正文 JSON 再导出及失败结果使用相同字段。没有适用值时填 `null`，没有记录的数组为空。

| 字段 | 内容 |
| --- | --- |
| `status` | `ok` 完成；`partial` 已导出，但有下载失败或正文、资源完整性问题；`error` 运行失败 |
| `code`、`error`、`network` | 运行失败的编号、说明和网络详情；成功及部分成功时为 `null` |
| `source`、`output` | 输入来源和输出目录 |
| `selector`、`requested_selector` | 实际及请求的正文选择器；正文 JSON 再导出时为 `null` |
| `counts` | 正文块（含嵌套块）、图片和附件数量；失败时为 `null` |
| `warnings`、`notes` | 分别保存警告和提示，包含编号与具体信息 |
| `image_errors`、`attachment_errors` | 图片、附件下载失败记录，也包含在 `warnings` 中 |
| `content_integrity`、`resource_integrity` | 文字顺序与资源完整性检查结果；未检查时为 `null` |
| `output_profile`、`output_options` | 输出预设和实际开关 |
| `output_changes` | 本次输出记录的格式调整 |

`ok` 和 `partial` 的退出码为 0，运行失败为 1，参数解析失败为 2。自动验收时检查 `status`，并查看 `image_errors`、`attachment_errors` 和完整性结果。普通样式警告不会将状态改为 `partial`。文章内容保存在 `reconstructed.json` 中。

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
