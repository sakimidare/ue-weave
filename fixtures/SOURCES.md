# 真实页面回归样本

这些文件来自本次真实 HTTP 抓取保存的正文快照。测试使用本地内容，运行时不访问网络。

- `seu-585911.html`：https://jwc.seu.edu.cn/2026/1013/c71422a585911/page.htm
- `seu-583204.html`：https://jwc.seu.edu.cn/2026/0915/c21679a583204/page.htm
- `seu-583286.html`：https://jwc.seu.edu.cn/2026/0916/c21678a583286/page.htm

正文比较忽略 Unicode 空白，并按工具规则移除已知 undefinedundefined 残留；其他字符必须按顺序完全一致。空白保真另由专门测试覆盖。此比较不证明字体或页面布局相同。

## 多站点模板样本

以下完整页面于 2026-11-03 从公开站点保存，文件统一使用 LF 换行。测试读取 HTML，页面中的脚本作为静态源码保存。

| 文件 | 页面 | 覆盖内容 |
| --- | --- | --- |
| `xsxy-534301.html` | https://xsxy.seu.edu.cn/2025/0624/c57168a534301/page.htm | 纯图片正文、标题和日期 |
| `cse-588376.html` | https://cse.seu.edu.cn/2026/1102/c22535a588376/page.htm | 学院新闻、图文混排 |
| `cse-585944.html` | https://cse.seu.edu.cn/2026/1014/c49342a585944/page.htm | 附件链接、附件图标 |
| `ee-586046.html` | https://ee.seu.edu.cn/2026/1015/c25183a586046/page.htm | 电气学院模板、导航栏同名标题干扰 |
| `ee-581788.html` | https://ee.seu.edu.cn/2026/0907/c25184a581788/page.htm | 电气学院通知、内嵌 PDF 附件 |

`seu-578974.html` 来自此前验收目录 `002-578974/original.html`，对应 https://jwc.seu.edu.cn/2026/0810/c21676a578974/page.htm ，覆盖合并单元格和附件。该文件只含正文，元数据缺失测试直接使用这份快照。另有测试从电气学院完整样本移除文章元数据，验证导航内容不会被误识别为标题。
