use serde_json::Value;
mod common;

#[test]
fn report_schema_is_shared_by_html_ir_and_failure() {
    let dir = common::directory();
    let run = |args: &[&str], output: &std::path::Path| {
        let result = common::command()
            .args(args)
            .args([
                "--print-json-result",
                "--log",
                "off",
                "--formats",
                "ir",
                "--output",
            ])
            .arg(output)
            .output()
            .unwrap();
        serde_json::from_slice::<Value>(&result.stdout).unwrap()
    };
    let first = dir.path().join("first");
    let html = run(&["--input", "fixtures/crawl-regressions.html"], &first);
    let ir = run(
        &[
            "--input-ir",
            first.join("reconstructed.json").to_str().unwrap(),
        ],
        &dir.path().join("second"),
    );
    let error = run(&["--input", "missing-report-fixture.html"], &first);
    let keys = |report: &Value| {
        report
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect::<Vec<_>>()
    };
    assert_eq!(keys(&html), keys(&ir));
    assert_eq!(keys(&html), keys(&error));
    assert_eq!(html["status"], "ok");
    assert_eq!(ir["status"], "ok");
    assert_eq!(error["status"], "error");
    assert_eq!(error["code"], "E0005");
    assert!(!html["notes"].as_array().unwrap().is_empty());
    assert!(!html["warnings"].as_array().unwrap().is_empty());
    for (key, level) in [("warnings", "warning"), ("notes", "note")] {
        assert!(html[key]
            .as_array()
            .unwrap()
            .iter()
            .all(|item| item["level"] == level));
    }
}

#[test]
fn nonexistent_log_codes_are_errors() {
    for code in ["W9999", "N9999", "E0001"] {
        let dir = common::directory();
        let result = common::command()
            .args([
                "--input",
                "fixtures/crawl-regressions.html",
                "--print-json-result",
                "--log",
            ])
            .arg(format!("{code}=off"))
            .arg("--output")
            .arg(dir.path())
            .output()
            .unwrap();
        assert!(!result.status.success());
        let report: Value = serde_json::from_slice(&result.stdout).unwrap();
        assert_eq!(report["code"], "E0017");
    }
}

#[test]
fn visible_style_losses_warn_in_all_profiles() {
    let dir = common::directory();
    let input = dir.path().join("styles.html");
    std::fs::write(&input, "<div class='Article_Content'><p style='opacity:0;position:absolute;overflow:hidden;transform:scale(0)'>正文</p></div>").unwrap();
    for profile in ["conservative", "balanced", "aggressive"] {
        let result = common::command()
            .arg("--input")
            .arg(&input)
            .args([
                "--profile",
                profile,
                "--print-json-result",
                "--log",
                "off",
                "--output",
            ])
            .arg(dir.path().join(profile))
            .output()
            .unwrap();
        assert!(result.status.success());
        let report: Value = serde_json::from_slice(&result.stdout).unwrap();
        for attribute in ["opacity", "position", "overflow", "transform"] {
            assert!(
                report["warnings"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|w| w["attribute"] == attribute),
                "{profile}: {attribute}"
            );
        }
        assert_eq!(report["status"], "ok");
    }
}
