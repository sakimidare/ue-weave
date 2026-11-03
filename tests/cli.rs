use std::{fs, process::Command};

#[test]
fn webplus_auto_selection_and_snapshot() {
    let dir = std::env::temp_dir().join(format!("ue-webplus-{}", std::process::id()));
    let output = Command::new(env!("CARGO_BIN_EXE_ue-recover"))
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
    let explicit = Command::new(env!("CARGO_BIN_EXE_ue-recover"))
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
        let output = Command::new(env!("CARGO_BIN_EXE_ue-recover"))
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
    let result = Command::new(env!("CARGO_BIN_EXE_ue-recover"))
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
    let result = Command::new(env!("CARGO_BIN_EXE_ue-recover"))
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
    let result = Command::new(env!("CARGO_BIN_EXE_ue-recover"))
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
    let result = Command::new(env!("CARGO_BIN_EXE_ue-recover"))
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
        let output = Command::new(env!("CARGO_BIN_EXE_ue-recover"))
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
    let output = Command::new(env!("CARGO_BIN_EXE_ue-recover"))
        .args(["--input", "fixtures/table-spans.html", "--output"])
        .arg(&output_dir)
        .args(["--json", "--verbose", "--keep-raw"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["counts"]["blocks"], 1);
    assert!(String::from_utf8_lossy(&output.stderr).contains("[ue-recover][done]"));
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
    let output = Command::new(env!("CARGO_BIN_EXE_ue-recover"))
        .args(["--output", "/tmp/ue-unused-output", "--json"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let error: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(error["status"], "error");
    assert!(error["error"].as_str().unwrap().contains("URL"));
}
