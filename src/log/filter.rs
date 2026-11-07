//! `--log` 开关的解析与诊断级别选择。

use crate::diagnostic::{topics, DiagnosticKind, NoteKind, Topic, WarningKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Off,
    Summary,
    List,
    Context,
}

impl Level {
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value.trim().to_ascii_lowercase().as_str() {
            "off" => Self::Off,
            "summary" => Self::Summary,
            "list" => Self::List,
            "context" => Self::Context,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone)]
enum Target {
    All,
    Severity(&'static str),
    Topic(Topic),
    TopicSeverity(Topic, &'static str),
    Code(String),
}

#[derive(Debug, Clone)]
struct Rule {
    target: Target,
    level: Level,
}

#[derive(Debug, Clone, Default)]
pub struct Filter {
    rules: Vec<Rule>,
}

const RANK_ALL: u8 = 0;
const RANK_SEVERITY: u8 = 1;
const RANK_TOPIC: u8 = 2;
const RANK_TOPIC_SEVERITY: u8 = 3;
const RANK_CODE: u8 = 4;

fn rank(target: &Target) -> u8 {
    match target {
        Target::All => RANK_ALL,
        Target::Severity(_) => RANK_SEVERITY,
        Target::Topic(_) => RANK_TOPIC,
        Target::TopicSeverity(_, _) => RANK_TOPIC_SEVERITY,
        Target::Code(_) => RANK_CODE,
    }
}

fn matches(target: &Target, kind: DiagnosticKind) -> bool {
    match target {
        Target::All => true,
        Target::Severity(level) => kind.level() == *level,
        Target::Topic(topic) => topics(kind).contains(topic),
        Target::TopicSeverity(topic, level) => {
            kind.level() == *level && topics(kind).contains(topic)
        }
        Target::Code(code) => kind.code().eq_ignore_ascii_case(code),
    }
}

fn severity(name: &str) -> Option<&'static str> {
    match name.trim().to_ascii_lowercase().as_str() {
        "warning" => Some("warning"),
        "note" => Some("note"),
        _ => None,
    }
}

fn is_code(name: &str) -> bool {
    let bytes = name.as_bytes();
    bytes.len() == 5
        && matches!(bytes[0].to_ascii_uppercase(), b'E' | b'W' | b'N')
        && bytes[1..].iter().all(u8::is_ascii_digit)
}

fn unknown(name: &str) -> String {
    format!(
        "未知日志类别：{name}；可用：{}；组合写法如 style.warning",
        crate::diagnostic::TOPIC_NAMES.join("、")
    )
}

fn parse_target(name: &str) -> Result<Target, String> {
    let name = name.trim();
    if name.eq_ignore_ascii_case("all") {
        return Ok(Target::All);
    }
    if let Some(level) = severity(name) {
        return Ok(Target::Severity(level));
    }
    if is_code(name) {
        if crate::explain::lookup(name).is_err() {
            return Err(format!("日志编号不存在：{name}；例如 W0014、N0037"));
        }
        if name.starts_with(['E', 'e']) {
            return Err(format!("错误编号 {name} 始终显示；请选择 W 或 N 编号"));
        }
        return Ok(Target::Code(name.to_ascii_uppercase()));
    }
    if let Some((topic, level)) = name.split_once('.') {
        let topic = Topic::parse(topic.trim()).ok_or_else(|| unknown(name))?;
        let level = severity(level).ok_or_else(|| {
            format!(
                "未知严重度：{}；可用：warning、note，例如 style.warning=list",
                level.trim()
            )
        })?;
        return Ok(Target::TopicSeverity(topic, level));
    }
    Topic::parse(name)
        .map(Target::Topic)
        .ok_or_else(|| unknown(name))
}

fn parse(spec: &str) -> Result<Rule, String> {
    let Some((name, level)) = spec.split_once('=') else {
        // 省略类别时作用于全部诊断，例如 `--log summary`。
        return Level::parse(spec)
            .map(|level| Rule {
                target: Target::All,
                level,
            })
            .ok_or_else(|| {
                format!(
                    "日志开关无效：{spec}；格式为 [topic[.severity]=]level，例如 --log style=list"
                )
            });
    };
    let level = Level::parse(level).ok_or_else(|| {
        format!(
            "未知日志级别：{}；可用：off、summary、list、context",
            level.trim()
        )
    })?;
    Ok(Rule {
        target: parse_target(name)?,
        level,
    })
}

impl Filter {
    pub fn from_specs(specs: &[String], quiet: bool, verbose: bool) -> Result<Self, String> {
        let mut filter = Self::default();
        if quiet {
            filter.rules.push(Rule {
                target: Target::All,
                level: Level::Off,
            });
        }
        if verbose {
            filter.rules.push(Rule {
                target: Target::All,
                level: Level::Context,
            });
        }
        for spec in specs {
            for part in spec.split(',') {
                let part = part.trim();
                if part.is_empty() {
                    continue;
                }
                filter.rules.push(parse(part)?);
            }
        }
        Ok(filter)
    }

    /// warning 默认按编号聚合；带地址或位置的编号默认逐条展开；提示默认关闭。
    fn default_level(kind: DiagnosticKind) -> Level {
        match kind {
            DiagnosticKind::Warning(kind) => match kind {
                WarningKind::DecodeReplacement
                | WarningKind::ContentIntegrityMismatch
                | WarningKind::ResourceIntegrityMismatch
                | WarningKind::ImageDownload
                | WarningKind::AttachmentDownload
                | WarningKind::AssetFormat => Level::List,
                WarningKind::UnknownEncoding
                | WarningKind::AttachmentMetadataUnparsed
                | WarningKind::InvalidBaseUrl
                | WarningKind::UnsupportedStructure
                | WarningKind::ExportLimitation
                | WarningKind::MalformedStyle
                | WarningKind::UnsafeAttribute
                | WarningKind::UnsafeStyle
                | WarningKind::UnmappedStyle
                | WarningKind::PossibleEditorFillCharacter
                | WarningKind::ExcludedElement
                | WarningKind::CustomListMarker
                | WarningKind::NestingLimit => Level::Summary,
            },
            DiagnosticKind::Note(kind) => match kind {
                // 运行时提示：默认显示，但可被 --quiet、note=off 或编号开关关闭。
                NoteKind::NetworkRetry | NoteKind::VerboseHint | NoteKind::ExplainHint => {
                    Level::List
                }
                _ => Level::Off,
            },
        }
    }

    pub fn level(&self, kind: DiagnosticKind) -> Level {
        let mut value = Self::default_level(kind);
        for layer in 0..=RANK_CODE {
            for rule in &self.rules {
                if rank(&rule.target) == layer && matches(&rule.target, kind) {
                    value = rule.level;
                }
            }
        }
        value
    }

    pub fn progress(&self) -> Level {
        let mut value = Level::Off;
        for layer in 0..=RANK_TOPIC {
            for rule in &self.rules {
                if rank(&rule.target) != layer {
                    continue;
                }
                match rule.target {
                    Target::All => value = rule.level,
                    Target::Topic(Topic::Progress) => value = rule.level,
                    _ => {}
                }
            }
        }
        // 进度行是连续输出，没有“聚合计数”这一档。
        if value == Level::Summary {
            Level::Off
        } else {
            value
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostic::{NoteKind, WarningKind};

    fn build(specs: &[&str]) -> Filter {
        let specs: Vec<String> = specs.iter().map(|s| (*s).to_string()).collect();
        Filter::from_specs(&specs, false, false).unwrap()
    }

    #[test]
    fn defaults_keep_warning_and_note_levels() {
        let filter = Filter::default();
        assert_eq!(
            filter.level(WarningKind::UnmappedStyle.into()),
            Level::Summary
        );
        assert_eq!(filter.level(WarningKind::ImageDownload.into()), Level::List);
        assert_eq!(filter.level(NoteKind::EmptyElement.into()), Level::Off);
        assert_eq!(filter.progress(), Level::Off);
    }

    #[test]
    fn topics_and_severities_intersect() {
        let filter = build(&["style.warning=list", "style.note=off"]);
        assert_eq!(filter.level(WarningKind::UnmappedStyle.into()), Level::List);
        assert_eq!(filter.level(NoteKind::IgnoredStyle.into()), Level::Off);

        let filter = build(&["style=list"]);
        assert_eq!(filter.level(WarningKind::UnmappedStyle.into()), Level::List);
        assert_eq!(filter.level(NoteKind::IgnoredStyle.into()), Level::List);

        let filter = build(&["warning=list"]);
        assert_eq!(filter.level(NoteKind::IgnoredStyle.into()), Level::Off);
    }

    #[test]
    fn one_domain_covers_its_warnings_and_notes() {
        let filter = build(&["attachment=list"]);
        assert_eq!(
            filter.level(WarningKind::AttachmentDownload.into()),
            Level::List
        );
        assert_eq!(filter.level(NoteKind::AttachmentIcon.into()), Level::List);
        assert_eq!(filter.level(NoteKind::ImageDimension.into()), Level::Off);
    }

    #[test]
    fn precedence_is_code_then_topic_severity_then_topic_then_severity_then_all() {
        let filter = build(&["all=context", "warning=off", "style=list"]);
        assert_eq!(filter.level(NoteKind::EmptyElement.into()), Level::Context);
        assert_eq!(filter.level(WarningKind::UnmappedStyle.into()), Level::List);
        assert_eq!(
            filter.level(WarningKind::UnsupportedStructure.into()),
            Level::Off
        );

        let filter = build(&["style=list", "style.warning=off", "W0014=list"]);
        assert_eq!(filter.level(WarningKind::UnmappedStyle.into()), Level::List);
        assert_eq!(filter.level(WarningKind::UnsafeStyle.into()), Level::Off);
        assert_eq!(filter.level(NoteKind::IgnoredStyle.into()), Level::List);
    }

    #[test]
    fn progress_is_independent_of_diagnostics() {
        let filter = build(&["progress=list"]);
        assert_eq!(filter.progress(), Level::List);
        assert_eq!(filter.level(NoteKind::EmptyElement.into()), Level::Off);

        let filter = Filter::from_specs(&[], false, true).unwrap();
        assert_eq!(filter.progress(), Level::Context);

        let filter = Filter::from_specs(&[], true, false).unwrap();
        assert_eq!(filter.progress(), Level::Off);
    }

    #[test]
    fn comma_separated_and_uppercase_codes_are_accepted() {
        let filter = build(&["style=off,w0014=list"]);
        assert_eq!(filter.level(WarningKind::UnmappedStyle.into()), Level::List);
        assert_eq!(filter.level(WarningKind::UnsafeStyle.into()), Level::Off);
    }

    #[test]
    fn invalid_specs_report_the_offending_part() {
        for (spec, expected) in [
            ("W9999=off", "日志编号不存在"),
            ("N9999=list", "日志编号不存在"),
            ("E0001=off", "始终显示"),
            ("style", "日志开关无效"),
            ("style=loud", "未知日志级别"),
            ("nonsense=list", "未知日志类别"),
            ("style.loud=list", "未知严重度"),
            ("asset=list", "未知日志类别"),
        ] {
            let error = Filter::from_specs(&[spec.to_string()], false, false).unwrap_err();
            assert!(error.contains(expected), "{spec}: {error}");
        }
    }

    #[test]
    fn level_without_topic_applies_to_everything() {
        let filter = build(&["summary"]);
        assert_eq!(filter.level(NoteKind::EmptyElement.into()), Level::Summary);
        assert_eq!(
            filter.level(WarningKind::UnmappedStyle.into()),
            Level::Summary
        );
    }
}
