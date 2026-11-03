# 已核对的源码规则

参考版本：fex-team/ueditor ea2585b483b9d3ead85b090928d72503c7107baf。

- `filterword.js` 的 `mso-margin-*-alt`、`mso-padding-*-alt`、`mso-width/height`：移植属性名映射，简写展开后进入段落／单元格字段；现存标准声明优先。保留输入单位，由 HTML 或导出器解释；未复制源码中可能造成精度损失的全局单位替换。

- `_parse/list.js`（全文）、`_src/plugins/list.js:206–245,485–535`：自定义类型由 `custom_cn/cn1/cn2/num/num1/num2/dash/dot` 类名指定，列表项使用 `list-cn-1-1` 等类名，通过可配置图标路径加载 GIF。IR 保存类型及来源证据；当前仓库无对应图标资源，输出基础列表并报告外观 unknown，不假定标记的括号和标点。

- `_src/core/Editor.js:783–800,913–975,1574–1630`：保存前运行输出规则，载入时运行输入规则；`enterTag=p` 时连续行内节点组成段落。当前按连续行内节点构建段落，并记录分组依据。站点自定义注册的规则及编辑器配置无法从保存结果确定。
- `_src/plugins/paste.js`（全文）：粘贴经过 `filterWord`、可配置 `filterRules`、输入规则和 `beforepaste`；纯文本模式另经 `filterTxtRules`。浏览器还会处理 Apple/moz 标记和尾部 br。最终 HTML 无法证明采用了哪个粘贴模式，不能据此补造已删除内容。
- `_parse/parse.js`（全文）：此文件提供展示插件注册、加载和 DOM 工具，`uParse` 调用已注册插件；`loadFile` 能加载额外资源。还原器不执行该代码或远程脚本；展示插件产生的动态内容不包含在静态 HTTP 快照中。
- `_src/core/domUtils.js:77`、`_src/core/htmlparser.js:60`：填充字符为 U+200B（IE6 为 U+FEFF），HTML 解析时删除当前浏览器对应字符。还原器遇到残留字符时报告来源 unknown，保留文本及节点证据；这些字符也可能是有意输入的内容。

- `_src/plugins/rowspacing.js`：段前段后间距通过 `margin-top`、`margin-bottom` 写入段落。当前保存最终 HTML 中这两项显式声明；原始 Word 间距仍为 unknown。

- `_src/core/filterword.js:44–69`、`_src/core/utils.js:835`：VML shape 图片转换。从 `v:imagedata src` 提取普通图片引用；shape 的 pt 宽高按 `round(value * 96 / 72)` 换算，px 宽高取整，记录转换依据。其他 VML 几何信息保存在原始节点中。

- `_src/core/domUtils.js:2033,2200`：空节点／空块判断排除 br、空白、填充字符，并检查非空元素。当前对含 br 的空段落作低置信度占位推断，保留原段落；`--no-inference` 关闭来源推断。
- `_src/core/filterword.js:25`：Word 检测使用 Mso、mso 属性及 Office 标记。当前来源判断为 medium，缺少标记时为 unknown。

- `_src/plugins/list.js:1–70`：列表支持 decimal、字母、罗马数字及自定义标记，`disablePInList` 默认关闭。IR 保留列表层级和列表项内段落；自定义标记的完整导出仍待补齐。

- `_src/plugins/indent.js`：首行缩进写入段落 `text-indent`，默认 `2em`，取消时为 `0em`。保留最终 HTML 中的显式值，不用默认值填充缺失声明。
- `_src/plugins/lineheight.js`：行距命令把 `1` 写成 `normal`，其他值写成 `em`。IR 记录可见声明及来源，HTML 输出保留安全的声明。

- `_src/plugins/basestyle.js`：bold 使用 strong/b，italic 使用 em/i，上下标使用 sup/sub。输入规则把 b/i 转为 strong/em。
- `_src/plugins/font.js:279`：输入规则把 u/s/del/strike 转成 span 的 text-decoration；font size 的 1–7 映射为 10/12/16/18/24/32/48px。Rust 已覆盖这些映射。
- `_src/core/filterword.js`：已移植存留的 horiz-align、vert-align、mso-vertical-align-alt、font-color、mso-foreground、mso-background、mso-highlight、text-line-through 声明映射，并记录原值。其他 mso 属性、VML 和单位转换仍待补齐。
- `_src/core/filternode.js`：按规则保留属性，未知普通元素可移除外层并保留子节点；CDATA 元素会删除。已阅读；当前 Rust 尚未完整实现该过滤机制。
- `_src/core/htmlparser.js`：处理填充字符、空白和列表/表格所需父节点。已阅读；Rust 使用 HTML5 解析器，行为存在差异。

本次回归测试覆盖字体大小映射、sup/sub、CSS 下划线与删除线、嵌套粗体取消及兄弟节点格式恢复。现存字体值表示最终 HTML 的可见声明，原始 Word 字体仍无法确认。
