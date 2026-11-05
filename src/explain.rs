use crate::error::{RecoverError, Result};

const GUIDE: &str = include_str!("../docs/explanations.md");

pub(crate) fn print(code: &str, color: crate::log::Color) -> Result<()> {
    use std::io::Write;
    let entry = lookup(code)?;
    let (title, body) = entry.split_once('\n').unwrap_or((entry, ""));
    let shade = match title.as_bytes()[0] {
        b'E' => anstyle::AnsiColor::Red,
        b'W' => anstyle::AnsiColor::Yellow,
        _ => anstyle::AnsiColor::Cyan,
    };
    let style = shade.on_default().bold();
    let mut stdout = anstream::AutoStream::new(std::io::stdout(), color.choice());
    writeln!(stdout, "{style}{title}{style:#}\n{body}").map_err(|source| {
        RecoverError::WriteOutput {
            path: "stdout".into(),
            source,
        }
    })
}

/// 解释正文随程序编译，并与仓库文档共用同一份内容。
pub fn lookup(code: &str) -> Result<&'static str> {
    let code = code.trim().to_ascii_uppercase();
    GUIDE
        .split("\n## ")
        .skip(1)
        .find(|entry| entry.split_whitespace().next() == Some(code.as_str()))
        .map(str::trim)
        .ok_or(RecoverError::UnknownDiagnosticCode { code })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn every_declared_code_has_an_example_and_action() {
        let mut codes = BTreeSet::new();
        for source in [
            include_str!("error.rs"),
            include_str!("diagnostic/warning.rs"),
            include_str!("diagnostic/note.rs"),
        ] {
            for literal in source.split('"').skip(1).step_by(2) {
                if literal.len() == 5
                    && matches!(literal.as_bytes()[0], b'E' | b'W' | b'N')
                    && literal.as_bytes()[1..].iter().all(u8::is_ascii_digit)
                {
                    codes.insert(literal);
                    let entry = lookup(literal).unwrap();
                    assert!(entry.contains("例子："), "{literal}");
                    assert!(entry.contains("处理："), "{literal}");
                }
            }
        }
        let entries: Vec<_> = GUIDE.split("\n## ").skip(1).collect();
        assert_eq!(entries.len(), codes.len(), "解释有重复或未使用的编号");
        for entry in entries {
            assert!(codes.contains(entry.split_whitespace().next().unwrap()));
        }
        let mut indexed = BTreeSet::new();
        for row in include_str!("../docs/diagnostics.md").lines() {
            let cells: Vec<_> = row.split('|').map(str::trim).collect();
            if cells.len() < 4 || !cells[1].starts_with(['E', 'W', 'N']) {
                continue;
            }
            assert!(codes.contains(cells[1]), "索引含未定义编号：{}", cells[1]);
            assert!(indexed.insert(cells[1]), "重复索引：{}", cells[1]);
            assert_eq!(
                lookup(cells[1]).unwrap().lines().next().unwrap(),
                format!("{} {}", cells[1], cells[2]),
                "索引标题与解释标题不一致"
            );
        }
        assert_eq!(indexed, codes, "诊断索引缺少编号");
    }
}
