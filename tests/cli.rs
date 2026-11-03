use std::{fs, process::Command};

#[test]
fn only_requested_final_documents_are_written() {
    let dir = std::env::temp_dir().join(format!("ue-final-only-{}", std::process::id()));
    for (name, formats, expected) in [
        (
            "default",
            None,
            vec![
                "reconstructed.html",
                "reconstructed.md",
                "reconstructed.typ",
            ],
        ),
        ("markdown", Some("markdown"), vec!["reconstructed.md"]),
        ("json", Some("json"), vec!["reconstructed.json"]),
    ] {
        let out = dir.join(name);
        let mut command = Command::new(env!("CARGO_BIN_EXE_ue-weave"));
        command
            .args(["--input", "fixtures/list-values.html", "--json", "--output"])
            .arg(&out);
        if let Some(formats) = formats {
            command.args(["--formats", formats]);
        }
        let result = command.output().unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&result.stdout).unwrap()["status"],
            "ok"
        );
        let mut actual = fs::read_dir(&out)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        actual.sort();
        assert_eq!(actual, expected);
    }
    let result = Command::new(env!("CARGO_BIN_EXE_ue-weave"))
        .arg("--input-ir")
        .arg(dir.join("json/reconstructed.json"))
        .args(["--formats", "html", "--output"])
        .arg(dir.join("edited"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(fs::read_dir(dir.join("edited")).unwrap().count(), 1);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn custom_asset_root_checks_paths_and_hashes() {
    use sha2::{Digest, Sha256};
    let dir = std::env::temp_dir().join(format!("ue-asset-root-{}", std::process::id()));
    let source = dir.join("source");
    let assets = dir.join("assets");
    fs::create_dir_all(assets.join("attachments")).unwrap();
    fs::write(assets.join("attachments/note.txt"), "附件").unwrap();
    let parsed = Command::new(env!("CARGO_BIN_EXE_ue-weave"))
        .args(["--formats", "html,markdown,typst,json"])
        .args(["--input", "fixtures/list-values.html", "--output"])
        .arg(&source)
        .output()
        .unwrap();
    assert!(parsed.status.success());
    let path = source.join("reconstructed.json");
    let mut ir: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    ir["attachments"] =
        serde_json::json!([{"local_path":"attachments/note.txt","download":{"sha256":"wrong"}}]);
    fs::write(&path, serde_json::to_vec(&ir).unwrap()).unwrap();
    let run = |target: &str| {
        Command::new(env!("CARGO_BIN_EXE_ue-weave"))
            .args(["--formats", "html,markdown,typst,json"])
            .arg("--input-ir")
            .arg(&path)
            .arg("--asset-root")
            .arg(&assets)
            .arg("--output")
            .arg(dir.join(target))
            .arg("--json")
            .output()
            .unwrap()
    };
    let failed = run("bad");
    assert!(!failed.status.success());
    assert!(String::from_utf8_lossy(&failed.stdout).contains("sha256"));
    ir["attachments"][0]["download"]["sha256"] =
        format!("{:x}", Sha256::digest("附件".as_bytes())).into();
    fs::write(&path, serde_json::to_vec(&ir).unwrap()).unwrap();
    assert!(run("good").status.success());
    assert_eq!(
        fs::read_to_string(dir.join("good/attachments/note.txt")).unwrap(),
        "附件"
    );
    ir["attachments"][0]["local_path"] = "../outside.txt".into();
    fs::write(&path, serde_json::to_vec(&ir).unwrap()).unwrap();
    assert!(!run("traversal").status.success());
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn metadata_header_is_optional_and_regenerates_from_edited_ir() {
    let dir = std::env::temp_dir().join(format!("ue-metadata-{}", std::process::id()));
    for (name, options) in [
        ("default", vec![]),
        ("enabled", vec!["--include-metadata"]),
        ("disabled", vec!["--include-metadata=false"]),
    ] {
        let result = Command::new(env!("CARGO_BIN_EXE_ue-weave"))
            .args(["--formats", "html,markdown,typst,json"])
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
    {
        let file = "reconstructed.json";
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
        .args(["--formats", "html,markdown,typst,json"])
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
    let invocation = std::cell::Cell::new(0);
    let run = |extra: &[&str]| {
        let index = invocation.get();
        invocation.set(index + 1);
        Command::new(env!("CARGO_BIN_EXE_ue-weave"))
            .args(["--formats", "html,markdown,typst,json"])
            .arg("--input")
            .arg(&html)
            .arg("--site-config")
            .arg(&config)
            .arg("--output")
            .arg(dir.join(format!("out-{index}")))
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
        serde_json::from_slice(&fs::read(dir.join("out-0/reconstructed.json")).unwrap()).unwrap();
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
        serde_json::from_slice(&fs::read(dir.join("out-2/reconstructed.json")).unwrap()).unwrap();
    assert_eq!(ir["metadata"]["title"], "默认标题");
    assert_eq!(ir["metadata"]["author"], "张三");
    assert!(!ir["metadata"].as_object().unwrap().contains_key("date"));
    assert_eq!(ir["blocks"][0]["text"], "手动正文");
    fs::write(&config, r#"{"content":"main","title":"h2"}"#).unwrap();
    let result = run(&[]);
    assert!(result.status.success());
    let ir: serde_json::Value =
        serde_json::from_slice(&fs::read(dir.join("out-3/reconstructed.json")).unwrap()).unwrap();
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
fn webplus_auto_selection_and_metadata() {
    let dir = std::env::temp_dir().join(format!("ue-webplus-{}", std::process::id()));
    let output = Command::new(env!("CARGO_BIN_EXE_ue-weave"))
        .args(["--formats", "html,markdown,typst,json"])
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
    let explicit = Command::new(env!("CARGO_BIN_EXE_ue-weave"))
        .args(["--formats", "html,markdown,typst,json"])
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
            .args(["--formats", "html,markdown,typst,json"])
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
            {
                let file = "reconstructed.json";
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
        .args(["--formats", "html,markdown,typst,json"])
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
fn distinguishes_selector_errors_without_writing_documents() {
    let directory = std::env::temp_dir().join(format!("ue-selector-errors-{}", std::process::id()));
    for (selector, code) in [
        ("[", "invalid_selector"),
        (".missing-article", "content_not_found"),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_ue-weave"))
            .args(["--formats", "html,markdown,typst,json"])
            .args(["--input", "fixtures/table-spans.html", "--output"])
            .arg(&directory)
            .args(["--selector", selector, "--json"])
            .output()
            .unwrap();
        assert!(!output.status.success());
        let error: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(error["code"], code);
        assert!(!directory.join("source.bin").exists());
    }
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn missing_input_emits_json_error_and_failure_exit() {
    let output = Command::new(env!("CARGO_BIN_EXE_ue-weave"))
        .args(["--formats", "html,markdown,typst,json"])
        .args(["--output", "/tmp/ue-unused-output", "--json"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let error: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(error["status"], "error");
    assert!(error["error"].as_str().unwrap().contains("URL"));
}
