pub struct Logger {
    verbose: bool,
}

impl Logger {
    pub fn retry(&self, error: &crate::error::NetworkError, delay: std::time::Duration) {
        eprintln!("[ue-weave][retry] {error}；{}ms 后重试", delay.as_millis());
    }
    pub fn new(verbose: bool) -> Self {
        Self { verbose }
    }
    pub fn stage(&self, name: &str, message: impl AsRef<str>) {
        if self.verbose {
            eprintln!("[ue-weave][{name}] {}", message.as_ref());
        }
    }
    pub fn done(&self, blocks: usize, attachments: usize, images: usize) {
        self.stage(
            "done",
            format!("完成：{blocks} 个正文块，{attachments} 个附件，{images} 张图片"),
        );
    }
    pub fn warnings(&self, warnings: &[serde_json::Value]) {
        if warnings.is_empty() {
            return;
        }
        let mut counts = std::collections::BTreeMap::new();
        for warning in warnings {
            let kind = warning["kind"].as_str().unwrap_or("unknown");
            *counts.entry(kind).or_insert(0usize) += 1;
            if self.verbose || kind.ends_with("-download") || kind == "decode-replacement" {
                let detail = warning["error"]
                    .as_str()
                    .or_else(|| warning["reason"].as_str())
                    .unwrap_or("详情请使用 --verbose");
                eprintln!(
                    "[ue-weave][warning][{kind}] {detail} {}",
                    warning["url"].as_str().unwrap_or("")
                );
            }
        }
        eprintln!(
            "[ue-weave][warnings] {} 条记录（{}）；详情可使用 --verbose 查看",
            warnings.len(),
            counts
                .iter()
                .map(|(kind, count)| format!("{kind}={count}"))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
}
