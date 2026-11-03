# 已核对的源码规则

参考版本：fex-team/ueditor ea2585b483b9d3ead85b090928d72503c7107baf。

- `filterword.js` 的 `mso-margin-*-alt`、`mso-padding-*-alt`、`mso-width/height`：移植属性名映射，简写展开后进入段落／单元格字段；现存标准声明优先。保留输入单位，由 HTML 或导出器解释。

- `_parse/list.js`（全文）、`_src/plugins/list.js:206–245,485–535`：自定义类型由 `custom_cn/cn1/cn2/num/num1/num2/dash/dot` 类名指定，列表项使用 `list-cn-1-1` 等类名，通过可配置图标路径加载 GIF。IR 保存类型及来源证据；导出基础列表。外部 GIF 图标资源缺失，标记外观记为 unknown。

- `_src/core/Editor.js:783–800,913–975,1574–1630`：保存前运行输出规则，载入时运行输入规则；`enterTag=p` 时连续行内节点组成段落。当前按连续行内节点构建段落，并记录分组依据。站点自定义规则及编辑器配置记为 unknown。
- `_src/plugins/paste.js`（全文）：粘贴经过 `filterWord`、可配置 `filterRules`、输入规则和 `beforepaste`；纯文本模式另经 `filterTxtRules`。浏览器还会处理 Apple/moz 标记和尾部 br。粘贴模式及过滤前的属性记为 unknown。
- `_parse/parse.js`（全文）：此文件提供展示插件注册、加载和 DOM 工具，`uParse` 调用已注册插件；`loadFile` 能加载额外资源。还原器解析静态 HTTP 快照中的 HTML 和属性；动态展示插件作为源码参考。
- `_src/core/domUtils.js:77`、`_src/core/htmlparser.js:60`：填充字符为 U+200B（IE6 为 U+FEFF），HTML 解析时删除当前浏览器对应字符。残留字符写入文本及原始节点，来源记为 unknown。

- `_src/plugins/rowspacing.js`：段前段后间距通过 `margin-top`、`margin-bottom` 写入段落。将 HTML 中这两项显式声明写入段落间距字段。

- `_src/core/filterword.js:44–69`、`_src/core/utils.js:835`：VML shape 图片转换。从 `v:imagedata src` 提取普通图片引用；shape 的 pt 宽高按 `round(value * 96 / 72)` 换算，px 宽高取整，记录转换依据。其他 VML 几何信息保存在原始节点中。

- `_src/core/domUtils.js:2033,2200`：空节点／空块判断排除 br、空白、填充字符，并检查非空元素。当前对含 br 的空段落作低置信度占位推断，保留原段落；`--no-inference` 关闭来源推断。
- `_src/core/filterword.js:25`：Word 检测使用 Mso、mso 属性及 Office 标记。当前来源判断为 medium，缺少标记时为 unknown。

- `_src/plugins/list.js:1–70`：列表支持 decimal、字母、罗马数字及自定义标记，`disablePInList` 默认关闭。IR 记录列表层级和列表项内段落；自定义标记通过基础列表导出。

- `_src/plugins/indent.js`：首行缩进写入段落 `text-indent`，默认 `2em`，取消时为 `0em`。记录 HTML 中的显式值；缺失声明按段落证据判定。
- `_src/plugins/lineheight.js`：行距命令把 `1` 写成 `normal`，其他值写成 `em`。IR 记录可见声明及来源，HTML 输出保留安全的声明。

- `_src/plugins/basestyle.js`：bold 使用 strong/b，italic 使用 em/i，上下标使用 sup/sub。输入规则把 b/i 转为 strong/em。
- `_src/plugins/font.js:279`：输入规则把 u/s/del/strike 转成 span 的 text-decoration；font size 的 1–7 映射为 10/12/16/18/24/32/48px。Rust 已覆盖这些映射。
- `_src/core/filterword.js`：已移植存留的 horiz-align、vert-align、mso-vertical-align-alt、font-color、mso-foreground、mso-background、mso-highlight、text-line-through 声明映射，并记录原值。其余属性记录在原始节点中，映射待实现。
- `_src/core/filternode.js`：按规则保留属性，未知普通元素可移除外层并保留子节点；CDATA 元素会删除。Rust 对已支持的节点和属性执行对应规则，其余规则映射待实现。
- `_src/core/htmlparser.js`：处理填充字符、空白和列表/表格所需父节点。Rust 使用 HTML5 解析器构建 DOM，并记录原始节点。

本次回归测试覆盖字体大小映射、sup/sub、CSS 下划线与删除线、嵌套粗体取消及兄弟节点格式恢复。字体字段记录 HTML 中的声明值，原始 Word 字体记为 unknown。
