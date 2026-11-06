use std::fs;

mod common;

fn run(input: &str, extra: &[&str]) -> std::process::Output {
    let temporary = common::directory();
    let mut command = common::command();
    command
        .args(["--input", input, "--output"])
        .arg(temporary.path())
        .args(["--formats", "json", "--color", "never"])
        .args(extra);
    command.output().unwrap()
}

#[test]
fn quiet_suppresses_summaries_but_keeps_errors() {
    let output = run("fixtures/crawl-regressions.html", &["--quiet"]);
    assert!(output.status.success(), "{:?}", output);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains("warning["), "{stderr}");
    assert!(!stderr.contains("note["), "{stderr}");

    let output = common::command()
        .args([
            "--input",
            "fixtures/missing-file.html",
            "--output",
            "/tmp/x",
        ])
        .arg("--quiet")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("error[E0005]"));
}

#[test]
fn log_topics_switch_between_summary_and_list() {
    let default = run("fixtures/crawl-regressions.html", &[]);
    let stderr = String::from_utf8_lossy(&default.stderr);
    assert!(stderr.contains("warning[W0014]"), "{stderr}");
    assert!(stderr.contains("（1 处）"), "{stderr}");

    let listed = run("fixtures/crawl-regressions.html", &["--log", "style=list"]);
    let stderr = String::from_utf8_lossy(&listed.stderr);
    assert!(stderr.contains("warning[W0014]"), "{stderr}");
    assert!(!stderr.contains("（1 处）"), "{stderr}");

    let off = run("fixtures/crawl-regressions.html", &["--log", "style=off"]);
    let stderr = String::from_utf8_lossy(&off.stderr);
    assert!(!stderr.contains("warning[W0014]"), "{stderr}");
}

#[test]
fn log_codes_and_notes_can_be_enabled_individually() {
    let listed = run("fixtures/crawl-regressions.html", &["--log", "w0014=list"]);
    let stderr = String::from_utf8_lossy(&listed.stderr);
    assert!(stderr.contains("warning[W0014]"), "{stderr}");

    let notes = run("fixtures/seu-585911.html", &["--log", "note=list"]);
    let stderr = String::from_utf8_lossy(&notes.stderr);
    assert!(stderr.contains("note[N"), "{stderr}");
}

#[test]
fn quiet_and_verbose_are_mutually_exclusive() {
    for args in [vec!["--quiet", "--verbose"], vec!["--verbose", "--quiet"]] {
        let output = common::command()
            .args(args)
            .args(["--input", "fixtures/crawl-regressions.html", "--output"])
            .arg(common::directory().path())
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2), "{:?}", output);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("E0001"), "{stderr}");
        assert!(stderr.contains("cannot be used with"), "{stderr}");
    }
}

#[test]
fn explicit_log_wins_over_quiet() {
    let output = run(
        "fixtures/crawl-regressions.html",
        &["--quiet", "--log", "note=list"],
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("note["), "{stderr}");
    assert!(!stderr.contains("warning["), "{stderr}");
}

#[test]
fn explicit_log_wins_over_verbose() {
    let output = run(
        "fixtures/crawl-regressions.html",
        &["--verbose", "--log", "style=off"],
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains("warning[W0014]"), "{stderr}");
    assert!(stderr.contains("Reading"), "{stderr}");
}

#[test]
fn topic_and_severity_can_be_combined() {
    let output = run(
        "fixtures/crawl-regressions.html",
        &["--log", "style.warning=list", "--log", "style.note=off"],
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("warning[W0014]"), "{stderr}");
    assert!(!stderr.contains("（1 处）"), "{stderr}");
    assert!(!stderr.contains("note[N0023]"), "{stderr}");

    let quiet_notes = run("fixtures/seu-585911.html", &["--log", "style.warning=off"]);
    let stderr = String::from_utf8_lossy(&quiet_notes.stderr);
    assert!(!stderr.contains("warning[W0014]"), "{stderr}");
}

#[test]
fn attachment_topic_covers_its_warnings_and_notes() {
    let output = run(
        "fixtures/crawl-regressions.html",
        &["--log", "attachment=list"],
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("note[N0007]"), "{stderr}");
}

#[test]
fn old_asset_and_media_names_are_rejected() {
    for name in ["asset", "media"] {
        let output = run(
            "fixtures/crawl-regressions.html",
            &["--log", &format!("{name}=list")],
        );
        assert!(!output.status.success(), "{name}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("E0017"),
            "{name}"
        );
    }
}

#[test]
fn failure_paths_respect_log_switches() {
    let temporary = common::directory();
    let dir = temporary.path();
    for (args, expect_hint) in [
        (vec!["--log", "N0030=off"], false),
        (vec!["--log", "all=off"], false),
        (vec!["--quiet"], false),
        (vec![], true),
    ] {
        let output = common::command()
            .args(["--input", "fixtures/does-not-exist.html", "--output"])
            .arg(dir)
            .args(["--color", "never"])
            .args(&args)
            .output()
            .unwrap();
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("error[E0005]"), "{args:?}: {stderr}");
        assert_eq!(
            stderr.contains("hint[N0030]"),
            expect_hint,
            "{args:?}: {stderr}"
        );
    }
}

#[test]
fn clap_errors_respect_log_switches() {
    let hidden = common::command()
        .args(["--log", "N0030=off", "--bad-flag"])
        .output()
        .unwrap();
    assert_eq!(hidden.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&hidden.stderr);
    assert!(stderr.contains("E0001"), "{stderr}");
    assert!(!stderr.contains("hint[N0030]"), "{stderr}");

    let shown = common::command().args(["--bad-flag"]).output().unwrap();
    assert!(String::from_utf8_lossy(&shown.stderr).contains("hint[N0030]"));
}

#[test]
fn bare_level_applies_to_every_diagnostic() {
    let output = run("fixtures/crawl-regressions.html", &["--log", "summary"]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("（1 处）"), "{stderr}");
}

#[test]
fn runtime_hints_can_be_disabled_by_code() {
    let output = run(
        "fixtures/crawl-regressions.html",
        &["--log", "all=off,W0014=summary,N0029=off,N0030=off"],
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("warning[W0014]"), "{stderr}");
    assert!(!stderr.contains("note[N0029]"), "{stderr}");
    assert!(!stderr.contains("hint[N0030]"), "{stderr}");
}

#[test]
fn summary_keeps_the_level_label_for_notes() {
    let output = run(
        "fixtures/seu-585911.html",
        &["--log", "note=summary", "--log", "warning=off"],
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("note[N0037]"), "{stderr}");
    assert!(!stderr.contains("warning[N0037]"), "{stderr}");
    assert!(!stderr.contains("warning["), "{stderr}");
}

#[test]
fn progress_lines_are_independent_of_diagnostics() {
    let output = run(
        "fixtures/crawl-regressions.html",
        &["--log", "progress=list"],
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Reading"), "{stderr}");
    assert!(stderr.contains("（1 处）"), "{stderr}");
    assert!(!stderr.contains("note[N0014]"), "{stderr}");
}

#[test]
fn invalid_log_switch_reports_dedicated_code() {
    for spec in ["style=loud", "stylesheet=list", "style"] {
        let output = run(
            "fixtures/crawl-regressions.html",
            &["--log", spec, "--json"],
        );
        assert!(!output.status.success(), "{spec}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("E0017"), "{spec}: {stderr}");
        let body: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(body["code"], "E0017", "{spec}");
    }
}

#[test]
fn log_switches_do_not_change_json_payload() {
    let temporary = common::directory();
    let base = common::command()
        .args(["--input", "fixtures/crawl-regressions.html", "--output"])
        .arg(temporary.path().join("plain"))
        .args(["--formats", "json", "--json", "--color", "never"])
        .output()
        .unwrap();
    let loud = common::command()
        .args(["--input", "fixtures/crawl-regressions.html", "--output"])
        .arg(temporary.path().join("loud"))
        .args(["--formats", "json", "--json", "--color", "never"])
        .args(["--log", "all=context"])
        .output()
        .unwrap();
    let plain: serde_json::Value = serde_json::from_slice(&base.stdout).unwrap();
    let loud: serde_json::Value = serde_json::from_slice(&loud.stdout).unwrap();
    assert_eq!(plain["warnings"], loud["warnings"]);
    assert_eq!(plain["counts"], loud["counts"]);
    let a = fs::read(temporary.path().join("plain/reconstructed.json")).unwrap();
    let b = fs::read(temporary.path().join("loud/reconstructed.json")).unwrap();
    assert_eq!(a, b);
}
