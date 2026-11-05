mod common;
use std::fs;

#[test]
fn explain_hint_finishes_warning_and_error_output_once() {
    let temporary = common::directory();
    let input = temporary.path().join("page.html");
    fs::write(&input, "<div class='Article_Content'><p onclick='alert(1)'>通知</p><p style='text-shadow:1px 1px red'>正文</p></div>").unwrap();
    for verbose in [false, true] {
        let mut command = common::command();
        command
            .arg("--input")
            .arg(&input)
            .arg("--output")
            .arg(temporary.path().join("out"))
            .args(["--json", "--color", "never"]);
        if verbose {
            command.arg("--verbose");
        }
        let result = command.output().unwrap();
        assert!(result.status.success());
        let report: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
        let warning = report["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["level"] == "warning")
            .unwrap();
        let text = String::from_utf8(result.stderr).unwrap();
        assert_eq!(text.matches("hint[N0030]").count(), 1);
        assert!(text.trim_end().lines().last().unwrap().contains(&format!(
            "ue-weave --explain {}",
            warning["code"].as_str().unwrap()
        )));
    }
    for (args, code) in [
        (vec!["--json", "--bad-option"], "E0001"),
        (vec!["--explain", "E00011", "--color", "never"], "E0011"),
    ] {
        let output = common::command().args(args).output().unwrap();
        assert!(!output.status.success());
        let text = String::from_utf8(output.stderr).unwrap();
        assert_eq!(text.matches("hint[N0030]").count(), 1);
        assert!(text
            .trim_end()
            .lines()
            .last()
            .unwrap()
            .contains(&format!("--explain {code}")));
    }
}

#[test]
fn note_only_and_explanations_have_no_hint() {
    let temporary = common::directory();
    let output = common::command()
        .args([
            "--input",
            "fixtures/webplus-college.html",
            "--verbose",
            "--output",
        ])
        .arg(temporary.path())
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8(output.stderr).unwrap();
    assert!(text.contains("note["));
    assert!(!text.contains("hint[N0030]"));
    let output = common::command()
        .args(["--explain", "W0005"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
}

#[test]
fn explain_colors_only_the_code_and_title() {
    for (code, shade) in [
        ("E0109", anstyle::AnsiColor::Red),
        ("W0005", anstyle::AnsiColor::Yellow),
        ("N0002", anstyle::AnsiColor::Cyan),
    ] {
        for color in ["auto", "never", "always"] {
            let output = common::command()
                .args(["--explain", code, "--color", color])
                .output()
                .unwrap();
            assert!(output.status.success(), "{:?}", output);
            let text = String::from_utf8(output.stdout).unwrap();
            let (title, body) = text.split_once('\n').unwrap();
            assert!(!body.contains('\u{1b}'));
            if color == "always" {
                let style = shade.on_default().bold();
                assert!(title.starts_with(&format!("{style}{code}")));
                assert!(title.ends_with(&format!("{style:#}")));
            } else {
                assert!(!title.contains('\u{1b}'));
            }
        }
    }
}

#[test]
fn error_reporting_uses_clap_argument_boundaries() {
    for args in [
        vec!["--", "--json"],
        vec!["--input=--json"],
        vec!["--input=--color=always"],
        vec!["--color=never", "--", "--json"],
    ] {
        let output = common::command().args(args).output().unwrap();
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.contains(&27));
        assert!(String::from_utf8_lossy(&output.stderr).contains("E0001"));
    }
    let output = common::command()
        .args(["--explain", "E00011"])
        .output()
        .unwrap();
    assert!(!String::from_utf8_lossy(&output.stderr).contains(".md"));
}

#[test]
fn explain_runs_alone_and_covers_errors_warnings_and_notes() {
    let temporary = common::directory();
    let codes = include_str!("../docs/explanations.md")
        .lines()
        .filter_map(|line| line.strip_prefix("## "))
        .map(|line| line.split_whitespace().next().unwrap())
        .chain(["n0002"]);
    for code in codes {
        let output = common::command()
            .current_dir(temporary.path())
            .args(["--explain", code, "--color", "never"])
            .output()
            .unwrap();
        assert!(output.status.success(), "{:?}", output);
        assert!(output.stderr.is_empty());
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(text.starts_with(&code.to_ascii_uppercase()));
        assert!(text.contains("例子："));
        assert!(text.contains("处理："));
    }
}

#[test]
fn explain_errors_are_typed_and_clap_validates_arguments() {
    for (args, code) in [
        (vec!["--explain", "N9999"], "E0011"),
        (vec!["--explain"], "E0001"),
        (
            vec!["--explain", "N0002", "--input", "missing.html"],
            "E0001",
        ),
        (vec!["--explain", "N0002", "--output", "unused"], "E0001"),
        (vec!["--input", "missing.html"], "E0001"),
    ] {
        let output = common::command().args(args).output().unwrap();
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains(code));
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn node_reference_errors_use_same_code_in_terminal_and_json() {
    let temporary = common::directory();
    let dir = temporary.path();
    let input = dir.join("article.json");
    fs::write(
        &input,
        serde_json::to_vec(&serde_json::json!({
            "schema": {"name":"ue-weave-ir","version":2},
            "metadata": {}, "blocks":[{"type":"paragraph", "source_nodes":["missing"]}]
        }))
        .unwrap(),
    )
    .unwrap();
    let output = common::command()
        .args(["--input-ir"])
        .arg(&input)
        .args(["--json", "--color", "never", "--output"])
        .arg(dir.join("out"))
        .output()
        .unwrap();
    assert!(!output.status.success());
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["code"], "E0310");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.starts_with("error[E0310]:"));
    assert!(stderr.contains("blocks[0].source_nodes"));
    assert!(stderr.contains("missing"));
}

#[test]
fn color_policy_keeps_json_plain_and_formats_errors() {
    for (color, colored) in [("auto", false), ("never", false), ("always", true)] {
        for invalid in [true, false] {
            let mut cmd = common::command();
            cmd.args(["--color", color, "--json"]);
            if invalid {
                cmd.arg("--bad-option");
            } else {
                cmd.args(["--output", "/tmp/ue-color-unused"]);
            }
            let output = cmd.output().unwrap();
            assert!(!output.status.success());
            assert_eq!(output.stderr.contains(&27), colored);
            assert!(!output.stdout.contains(&27));
            let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(result["code"], "E0001");
        }
    }
}

#[test]
fn invalid_profile_reports_clap_error_without_panicking() {
    for args in [
        vec!["--profile", "typo", "--output", "/tmp/ue-profile-invalid"],
        vec![
            "--profile",
            "typo",
            "--json",
            "--output",
            "/tmp/ue-profile-invalid-json",
        ],
    ] {
        let output = common::command().args(args).output().unwrap();
        assert_eq!(output.status.code(), Some(2), "stderr: {:?}", output);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("invalid value") || stderr.contains("possible values"));
        assert!(!stderr.contains("panicked at"));
        assert!(!stderr.contains("ArgAction"));
        if output.stdout.is_empty() {
            continue;
        }
        let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(result["status"], "error");
    }
}

#[test]
fn malformed_presentation_options_report_errors_without_panicking() {
    for args in [
        vec!["--color", "typo", "--output", "/tmp/ue-color-invalid"],
        vec!["--color", "--output", "/tmp/ue-color-missing"],
        vec![
            "--profile",
            "typo",
            "--color",
            "always",
            "--json",
            "--output",
            "/tmp/ue-profile-invalid-colored",
        ],
    ] {
        let output = common::command().args(args).output().unwrap();
        assert_eq!(output.status.code(), Some(2), "stderr: {:?}", output);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(!stderr.contains("panicked at"));
        assert!(!stderr.contains("ArgAction"));
        assert!(stderr.contains("error[E0001]:"));
    }
}

#[test]
fn numbered_diagnostics_match_terminal_and_json() {
    let invalid = common::command()
        .args(["--json", "--unknown-option"])
        .output()
        .unwrap();
    assert!(!invalid.status.success());
    let result: serde_json::Value = serde_json::from_slice(&invalid.stdout).unwrap();
    assert_eq!(result["code"], "E0001");
    assert!(String::from_utf8_lossy(&invalid.stderr).starts_with("error[E0001]:"));

    let temporary = common::directory();
    let dir = temporary.path();
    let output = common::command()
        .args([
            "--input",
            "fixtures/webplus-college.html",
            "--formats",
            "json",
            "--json",
            "--verbose",
            "--output",
        ])
        .arg(dir)
        .output()
        .unwrap();
    assert!(output.status.success());
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    let warnings = result["warnings"].as_array().unwrap();
    assert!(!warnings.is_empty());
    for warning in warnings {
        let code = warning["code"].as_str().unwrap();
        assert_eq!(code.len(), 5);
        let level = warning["level"].as_str().unwrap();
        assert!(stderr.contains(&format!("{level}[{code}]:")));
    }
    let article: serde_json::Value =
        serde_json::from_slice(&fs::read(dir.join("reconstructed.json")).unwrap()).unwrap();
    assert!(article.get("warnings").is_none());
}

#[test]
fn only_requested_final_documents_are_written() {
    let temporary = common::directory();
    let dir = temporary.path();
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
        let mut command = common::command();
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
    let result = common::command()
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
}

#[test]
fn custom_asset_root_checks_paths_and_content_fields() {
    let temporary = common::directory();
    let dir = temporary.path();
    let source = dir.join("source");
    let assets = dir.join("assets");
    fs::create_dir_all(assets.join("attachments")).unwrap();
    fs::write(assets.join("attachments/note.txt"), "附件").unwrap();
    let parsed = common::command()
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
        common::command()
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
    assert!(String::from_utf8_lossy(&failed.stdout).contains("download"));
    ir["attachments"][0]
        .as_object_mut()
        .unwrap()
        .remove("download");
    fs::write(&path, serde_json::to_vec(&ir).unwrap()).unwrap();
    assert!(run("good").status.success());
    assert_eq!(
        fs::read_to_string(dir.join("good/attachments/note.txt")).unwrap(),
        "附件"
    );
    ir["attachments"][0]["local_path"] = "../outside.txt".into();
    fs::write(&path, serde_json::to_vec(&ir).unwrap()).unwrap();
    assert!(!run("traversal").status.success());
}

#[test]
fn metadata_header_is_optional_and_regenerates_from_edited_ir() {
    let temporary = common::directory();
    let dir = temporary.path();
    for (name, options) in [
        ("default", vec![]),
        ("enabled", vec!["--include-metadata"]),
        ("disabled", vec!["--include-metadata=false"]),
    ] {
        let result = common::command()
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
    let result = common::command()
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
}

#[test]
fn site_config_overrides_defaults_and_validates_input() {
    let temporary = common::directory();
    let dir = temporary.path();
    let config = dir.join("sites.json");
    let html = dir.join("page.html");
    fs::write(&html, "<h1>站点标题</h1><h2>默认标题</h2><time>2026-11-03</time><b class='writer'>作者：张三</b><article><p>\
        自定义正文</p></article><main><p>手动正文</p></main>").unwrap();
    fs::write(
        &config,
        r#"{"content":"article","title":"h1","date":"time","author":".writer"}"#,
    )
    .unwrap();
    let invocation = std::cell::Cell::new(0);
    let run = |extra: &[&str]| {
        let index = invocation.get();
        invocation.set(index + 1);
        common::command()
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
    assert_eq!(ir["blocks"][0]["runs"][0]["text"], "自定义正文");
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
    assert_eq!(ir["blocks"][0]["runs"][0]["text"], "手动正文");
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
        assert_eq!(report["code"], "E0003");
        assert!(report["error"].as_str().unwrap().contains("sites.json"));
    }
    fs::remove_file(&config).unwrap();
    let report: serde_json::Value = serde_json::from_slice(&run(&[]).stdout).unwrap();
    assert_eq!(report["code"], "E0002");
}

#[test]
fn webplus_auto_selection_and_metadata() {
    let temporary = common::directory();
    let dir = temporary.path();
    let output = common::command()
        .args(["--formats", "html,markdown,typst,json"])
        .args(["--input", "fixtures/webplus-college.html", "--output"])
        .arg(dir)
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
    let explicit = common::command()
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
}

#[test]
fn output_profiles_and_overrides_preserve_canonical_ir() {
    let temporary = common::directory();
    let base = temporary.path();
    for profile in ["conservative", "balanced", "aggressive"] {
        let output = common::command()
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
    let result = common::command()
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
}

#[test]
fn distinguishes_selector_errors_without_writing_documents() {
    let temporary = common::directory();
    let directory = temporary.path().join("output");
    for (selector, code) in [("[", "E0006"), (".missing-article", "E0007")] {
        let output = common::command()
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
    assert!(!directory.exists());
}

#[test]
fn missing_input_emits_json_error_and_failure_exit() {
    let output = common::command()
        .args(["--formats", "html,markdown,typst,json"])
        .args(["--output", "/tmp/ue-unused-output", "--json"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let error: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(error["status"], "error");
    assert!(error["error"].as_str().unwrap().contains("URL"));
}
