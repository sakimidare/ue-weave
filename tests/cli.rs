use std::{fs, process::Command};

#[test]
fn metadata_header_is_optional_and_regenerates_from_edited_ir() {
    let dir = std::env::temp_dir().join(format!("ue-metadata-{}", std::process::id()));
    for (name, options) in [
        ("default", vec![]),
        ("enabled", vec!["--include-metadata"]),
        ("disabled", vec!["--include-metadata=false"]),
    ] {
        let result = Command::new(env!("CARGO_BIN_EXE_ue-weave"))
            .args([
                "--input",
                "fixtures/webplus-college.html",
                "--profile",
                "aggressive",
                "--output",
            ])
            .arg(dir.join(name))
            .args(options)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
    for file in ["reconstructed.json", "original.html", "source.bin"] {
        assert_eq!(
            fs::read(dir.join("default").join(file)).unwrap(),
            fs::read(dir.join("enabled").join(file)).unwrap()
        );
    }
    for file in [
        "reconstructed.html",
        "reconstructed.md",
        "reconstructed.typ",
    ] {
        let default = fs::read_to_string(dir.join("default").join(file)).unwrap();
        let enabled = fs::read_to_string(dir.join("enabled").join(file)).unwrap();
        assert_eq!(
            default,
            fs::read_to_string(dir.join("disabled").join(file)).unwrap()
        );
        assert!(enabled.ends_with(&default));
        assert!(enabled.find("学院新闻").unwrap() < enabled.find("活动安排").unwrap());
        assert!(enabled.contains("发布者：管理员"));
        assert!(!enabled.contains("作者："));
    }
    let path = dir.join("enabled/reconstructed.json");
    let mut ir: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    ir["metadata"]["title"] = "编辑后的标题".into();
    ir["metadata"]["author"] = "张三".into();
    fs::write(&path, serde_json::to_vec(&ir).unwrap()).unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_ue-weave"))
        .arg("--input-ir")
        .arg(path)
        .args(["--include-metadata", "--output"])
        .arg(dir.join("edited"))
        .output()
        .unwrap();
    assert!(result.status.success());
    for file in [
        "reconstructed.html",
        "reconstructed.md",
        "reconstructed.typ",
    ] {
        let text = fs::read_to_string(dir.join("edited").join(file)).unwrap();
        assert_eq!(text.matches("编辑后的标题").count(), 1);
        assert!(text.contains("作者：张三"));
    }
    let report: serde_json::Value =
        serde_json::from_slice(&fs::read(dir.join("edited/report.json")).unwrap()).unwrap();
    assert_eq!(report["output_options"]["include_metadata"], true);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn site_config_overrides_defaults_and_validates_input() {
    let dir = std::env::temp_dir().join(format!("ue-site-config-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let config = dir.join("sites.json");
    let html = dir.join("page.html");
    fs::write(&html, "<h1>站点标题</h1><h2>默认标题</h2><time>2026-11-03</time><b class='writer'>作者：张三</b><article><p>自定义正文</p></article><main><p>手动正文</p></main>").unwrap();
    fs::write(
        &config,
        r#"{"content":"article","title":"h1","date":"time","author":".writer"}"#,
    )
    .unwrap();
    let run = |extra: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_ue-weave"))
            .arg("--input")
            .arg(&html)
            .arg("--site-config")
            .arg(&config)
            .arg("--output")
            .arg(dir.join("out"))
            .arg("--json")
            .args(extra)
            .output()
            .unwrap()
    };
    let result = run(&[]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let ir: serde_json::Value =
        serde_json::from_slice(&fs::read(dir.join("out/reconstructed.json")).unwrap()).unwrap();
    assert_eq!(ir["metadata"]["title"], "站点标题");
    assert_eq!(ir["metadata"]["date"], "2026-11-03");
    assert_eq!(ir["metadata"]["author"], "张三");
    assert_eq!(ir["blocks"][0]["text"], "自定义正文");
    let result = run(&["--selector", "main"]);
    assert!(result.status.success());
    let report: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(report["selector"], "main");
    let result = run(&[
        "--selector",
        "content=main",
        "--selector",
        "title=h2",
        "--selector",
        "date=.missing",
    ]);
    assert!(result.status.success());
    let ir: serde_json::Value =
        serde_json::from_slice(&fs::read(dir.join("out/reconstructed.json")).unwrap()).unwrap();
    assert_eq!(ir["metadata"]["title"], "默认标题");
    assert_eq!(ir["metadata"]["author"], "张三");
    assert!(!ir["metadata"].as_object().unwrap().contains_key("date"));
    assert_eq!(ir["blocks"][0]["text"], "手动正文");
    fs::write(&config, r#"{"content":"main","title":"h2"}"#).unwrap();
    let result = run(&[]);
    assert!(result.status.success());
    let ir: serde_json::Value =
        serde_json::from_slice(&fs::read(dir.join("out/reconstructed.json")).unwrap()).unwrap();
    assert_eq!(ir["metadata"]["title"], "默认标题");
    assert!(!run(&["--site", "missing.example"]).status.success());

    for invalid in [
        "{broken",
        r#"{"titel":"h1"}"#,
        r#"{"content":"["}"#,
        r#"{"sites":{"https://college.example":{"title":"h1"}}}"#,
    ] {
        fs::write(&config, invalid).unwrap();
        let result = run(&[]);
        assert!(!result.status.success());
        let report: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
        assert_eq!(report["code"], "invalid_config");
        assert!(report["error"].as_str().unwrap().contains("sites.json"));
    }
    fs::remove_file(&config).unwrap();
    let report: serde_json::Value = serde_json::from_slice(&run(&[]).stdout).unwrap();
    assert_eq!(report["code"], "read_config");
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn webplus_auto_selection_and_snapshot() {
    let dir = std::env::temp_dir().join(format!("ue-webplus-{}", std::process::id()));
    let output = Command::new(env!("CARGO_BIN_EXE_ue-weave"))
        .args(["--input", "fixtures/webplus-college.html", "--output"])
        .arg(&dir)
        .arg("--json")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["selector"], ".wp_articlecontent");
    assert_eq!(report["requested_selector"], "auto");
    let ir: serde_json::Value =
        serde_json::from_slice(&fs::read(dir.join("reconstructed.json")).unwrap()).unwrap();
    assert_eq!(ir["metadata"]["title"], "学院新闻");
    assert_eq!(ir["metadata"]["publisher"], "管理员");
    assert!(ir["metadata"].get("author").is_none());
    assert_eq!(ir["metadata"]["date"], "2026-11-02");
    let raw = fs::read_to_string(dir.join("original.html")).unwrap();
    assert!(raw.contains("正文&nbsp;内容"));
    assert!(!raw.contains("arti_metas"));
    let explicit = Command::new(env!("CARGO_BIN_EXE_ue-weave"))
        .args([
            "--input",
            "fixtures/webplus-college.html",
            "--selector",
            ".missing",
            "--output",
        ])
        .arg(dir.join("missing"))
        .output()
        .unwrap();
    assert!(!explicit.status.success());
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn output_profiles_and_overrides_preserve_canonical_ir() {
    let base = std::env::temp_dir().join(format!("ue-profiles-{}", std::process::id()));
    for profile in ["conservative", "balanced", "aggressive"] {
        let output = Command::new(env!("CARGO_BIN_EXE_ue-weave"))
            .args([
                "--input",
                "fixtures/output-profiles.html",
                "--profile",
                profile,
                "--output",
            ])
            .arg(base.join(profile))
            .arg("--json")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(report["output_profile"], profile);
        if profile != "conservative" {
            for file in ["reconstructed.json", "original.html", "source.bin"] {
                assert_eq!(
                    fs::read(base.join(profile).join(file)).unwrap(),
                    fs::read(base.join("conservative").join(file)).unwrap(),
                    "{file}"
                );
            }
            for file in [
                "reconstructed.html",
                "reconstructed.md",
                "reconstructed.typ",
            ] {
                assert!(!fs::read_to_string(base.join(profile).join(file))
                    .unwrap()
                    .contains("微软雅黑"));
            }
            assert!(!report["output_changes"].as_array().unwrap().is_empty());
        }
    }
    let md = fs::read_to_string(base.join("aggressive/reconstructed.md")).unwrap();
    assert!(md.contains("3. 第一项"));
    assert!(md.contains("   - 嵌套项目"));
    assert!(md.contains("4. 第二项"));
    assert!(md.contains("| 名称 | 内容 |"));
    assert!(md.contains("A&#124;B"));
    assert!(md.contains("colspan=\"2\""));
    let result = Command::new(env!("CARGO_BIN_EXE_ue-weave"))
        .arg("--input-ir")
        .arg(base.join("conservative/reconstructed.json"))
        .args([
            "--profile",
            "激进",
            "--strip-fonts=false",
            "--strip-colors=false",
            "--drop-empty-paragraphs=false",
            "--output",
        ])
        .arg(base.join("override"))
        .arg("--json")
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(report["output_options"]["strip_fonts"], false);
    let html = fs::read_to_string(base.join("override/reconstructed.html")).unwrap();
    assert!(html.contains("微软雅黑") && html.contains("color:red") && html.contains("<br>"));
    fs::remove_dir_all(base).unwrap();
}

#[test]
fn edited_ir_regenerates_with_original_snapshot_and_assets() {
    let directory = std::env::temp_dir().join(format!("ue-ir-edit-{}", std::process::id()));
    let source = directory.join("source");
    let target = directory.join("edited");
    let result = Command::new(env!("CARGO_BIN_EXE_ue-weave"))
        .args(["--input", "fixtures/list-values.html", "--output"])
        .arg(&source)
        .output()
        .unwrap();
    assert!(result.status.success());
    let path = source.join("reconstructed.json");
    let mut ir: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    ir["blocks"][0]["children"][0]["children"][0]["runs"][0]["text"] = "编辑后的内容".into();
    fs::create_dir(source.join("attachments")).unwrap();
    fs::write(source.join("attachments/test.txt"), "附件内容").unwrap();
    ir["attachments"] = serde_json::json!([{"local_path":source.join("attachments/test.txt")}]);
    fs::write(&path, serde_json::to_vec(&ir).unwrap()).unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_ue-weave"))
        .arg("--input-ir")
        .arg(&path)
        .arg("--output")
        .arg(&target)
        .arg("--json")
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        fs::read(source.join("original.html")).unwrap(),
        fs::read(target.join("original.html")).unwrap()
    );
    assert_eq!(
        fs::read_to_string(target.join("attachments/test.txt")).unwrap(),
        "附件内容"
    );
    let restored: serde_json::Value =
        serde_json::from_slice(&fs::read(target.join("reconstructed.json")).unwrap()).unwrap();
    assert_eq!(
        restored["attachments"][0]["local_path"],
        "attachments/test.txt"
    );
    for name in [
        "reconstructed.html",
        "reconstructed.md",
        "reconstructed.typ",
    ] {
        assert!(fs::read_to_string(target.join(name))
            .unwrap()
            .contains("编辑后的内容"));
    }
    fs::write(&path, "{broken").unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_ue-weave"))
        .arg("--input-ir")
        .arg(&path)
        .arg("--output")
        .arg(&target)
        .arg("--json")
        .output()
        .unwrap();
    assert!(!result.status.success());
    let error: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(error["code"], "invalid_ir");
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn distinguishes_selector_errors_and_preserves_input_on_failure() {
    let directory = std::env::temp_dir().join(format!("ue-selector-errors-{}", std::process::id()));
    for (selector, code) in [
        ("[", "invalid_selector"),
        (".missing-article", "content_not_found"),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_ue-weave"))
            .args(["--input", "fixtures/table-spans.html", "--output"])
            .arg(&directory)
            .args(["--selector", selector, "--json"])
            .output()
            .unwrap();
        assert!(!output.status.success());
        let error: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(error["code"], code);
        assert_eq!(
            fs::read(directory.join("source.bin")).unwrap(),
            fs::read("fixtures/table-spans.html").unwrap()
        );
    }
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn json_stdout_and_full_source_snapshot() {
    let output_dir = std::env::temp_dir().join(format!(
        "ue-cli-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let output = Command::new(env!("CARGO_BIN_EXE_ue-weave"))
        .args(["--input", "fixtures/table-spans.html", "--output"])
        .arg(&output_dir)
        .args(["--json", "--verbose", "--keep-raw"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["counts"]["blocks"], 1);
    assert!(String::from_utf8_lossy(&output.stderr).contains("[ue-weave][done]"));
    assert_eq!(
        fs::read(output_dir.join("source.html")).unwrap(),
        fs::read("fixtures/table-spans.html").unwrap()
    );
    assert!(output_dir.join("original.html").is_file());
    assert_eq!(
        fs::read(output_dir.join("source.bin")).unwrap(),
        fs::read("fixtures/table-spans.html").unwrap()
    );
    fs::remove_dir_all(output_dir).unwrap();
}

#[test]
fn missing_input_emits_json_error_and_failure_exit() {
    let output = Command::new(env!("CARGO_BIN_EXE_ue-weave"))
        .args(["--output", "/tmp/ue-unused-output", "--json"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let error: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(error["status"], "error");
    assert!(error["error"].as_str().unwrap().contains("URL"));
}
