# 真实页面回归样本

这些文件来自本次真实 HTTP 抓取保存的正文快照。测试使用本地内容，运行时不访问网络。

- `seu-585911.html`：https://jwc.seu.edu.cn/2026/1013/c71422a585911/page.htm
- `seu-583204.html`：https://jwc.seu.edu.cn/2026/0915/c21679a583204/page.htm
- `seu-583286.html`：https://jwc.seu.edu.cn/2026/0916/c21678a583286/page.htm

正文比较忽略 Unicode 空白，并按工具规则移除已知 undefinedundefined 残留；其他字符必须按顺序完全一致。空白保真另由专门测试覆盖。此比较不证明字体或页面布局相同。
