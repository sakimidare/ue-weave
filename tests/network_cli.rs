use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    process::Command,
    thread,
    time::{Duration, Instant},
};

#[test]
fn quiet_suppresses_retry_notes_and_network_topic_restores_them() {
    let page = "<div class='Article_Content'><p>正文</p></div>";
    for (label, quiet, expect_retry) in [("quiet", true, false), ("explicit", false, true)] {
        let (url, server) = serve(vec![(503, "".into()), (200, page.into())]);
        let dir =
            std::env::temp_dir().join(format!("ue-http-retry-log-{}-{label}", std::process::id()));
        let mut command = Command::new(env!("CARGO_BIN_EXE_ue-weave"));
        command
            .arg(&url)
            .args([
                "--allow-private-network",
                "--retries",
                "1",
                "--timeout",
                "2",
                "--formats",
                "json",
                "--color",
                "never",
            ])
            .arg("--output")
            .arg(&dir)
            .env("NO_PROXY", "127.0.0.1")
            .env("no_proxy", "127.0.0.1");
        if quiet {
            command.arg("--quiet");
        } else {
            command.args(["--log", "network=list"]);
        }
        let result = command.output().unwrap();
        assert!(result.status.success(), "{label}");
        let stderr = String::from_utf8_lossy(&result.stderr);
        assert_eq!(stderr.contains("retry:"), expect_retry, "{label}: {stderr}");
        assert_eq!(server.join().unwrap(), 2);
        let _ = fs::remove_dir_all(dir);
    }
}

#[test]
fn fixed_address_preserves_host_and_downloads_mislabeled_images() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    listener.set_nonblocking(true).unwrap();
    let server = thread::spawn(move || {
        let bodies: [&[u8]; 2] = [
            b"<div class='Article_Content'><img src='/photo.png'></div>",
            b"\xff\xd8\xffjpeg-data",
        ];
        for (index, body) in bodies.iter().enumerate() {
            let deadline = Instant::now() + Duration::from_secs(10);
            let mut stream = loop {
                if let Ok((stream, _)) = listener.accept() {
                    break stream;
                }
                assert!(Instant::now() < deadline, "请求未到达");
                thread::sleep(Duration::from_millis(10));
            };
            let request = read_request(&mut stream);
            assert!(request
                .to_lowercase()
                .contains(&format!("host: campus.example:{port}")));
            let mime = if index == 0 { "text/html" } else { "image/png" };
            write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",body.len()).unwrap();
            stream.write_all(body).unwrap();
        }
    });
    let dir = tempfile::tempdir().unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_ue-weave"))
        .arg(format!("http://campus.example:{port}/page.htm"))
        .args([
            "--resolve",
            "campus.example=127.0.0.1",
            "--allow-private-network",
            "--download-images",
            "--json",
            "--output",
        ])
        .arg(dir.path())
        .output()
        .unwrap();
    server.join().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(report["image_errors"].as_array().unwrap().len(), 0);
    assert!(report["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|w| w["code"] == "W0015"));
    let html = fs::read_to_string(dir.path().join("reconstructed.html")).unwrap();
    assert!(html.contains("images/"));
    assert!(html.contains(".jpg"));
    let image = fs::read_dir(dir.path().join("images"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    assert_eq!(fs::read(image).unwrap(), b"\xff\xd8\xffjpeg-data");
}

#[test]
fn custom_headers_stay_on_the_input_origin() {
    for redirect in [false, true] {
        let source = TcpListener::bind("127.0.0.1:0").unwrap();
        let target = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", source.local_addr().unwrap());
        let target_url = format!("http://{}", target.local_addr().unwrap());
        let serve_request = |listener: TcpListener, response: String| {
            thread::spawn(move || {
                listener.set_nonblocking(true).unwrap();
                let deadline = Instant::now() + Duration::from_secs(5);
                let mut stream = loop {
                    if let Ok((stream, _)) = listener.accept() {
                        break stream;
                    }
                    assert!(Instant::now() < deadline, "未收到请求");
                    thread::sleep(Duration::from_millis(10));
                };
                let request = read_request(&mut stream);
                stream.write_all(response.as_bytes()).unwrap();
                request.to_ascii_lowercase()
            })
        };
        let body = format!("<div class='Article_Content'><img src='{target_url}/image.png'></div>");
        let response = if redirect {
            format!("HTTP/1.1 302 Found\r\nLocation: {target_url}/\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
        } else {
            format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
        };
        let first = serve_request(source, response);
        let body = "<div class='Article_Content'><p>正文</p></div>";
        let second = serve_request(
            target,
            format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            ),
        );
        let directory = tempfile::tempdir().unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_ue-weave"))
            .arg(&url)
            .args([
                "--allow-private-network",
                "--download-images",
                "--header",
                "Cookie=review_secret",
                "--header",
                "Authorization=Bearer review_secret",
                "--header",
                "X-Api-Key=review_secret",
                "--output",
            ])
            .arg(directory.path())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let first = first.join().unwrap();
        assert!(first.contains("cookie: review_secret"));
        assert!(first.contains("authorization: bearer review_secret"));
        assert!(first.contains("x-api-key: review_secret"));
        let second = second.join().unwrap();
        assert!(!second.contains("review_secret"), "{second}");
    }
}

#[test]
fn malformed_header_values_are_absent_from_diagnostics() {
    for header in ["Cookie: review_secret", "review_secret invalid=value"] {
        let directory = tempfile::tempdir().unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_ue-weave"))
            .args([
                "https://example.org/",
                "--header",
                header,
                "--json",
                "--output",
            ])
            .arg(directory.path())
            .output()
            .unwrap();
        assert!(!output.status.success());
        for bytes in [&output.stdout, &output.stderr] {
            let text = String::from_utf8_lossy(bytes);
            assert!(text.contains("E0014"));
            assert!(!text.contains("review_secret"));
        }
    }
}

#[test]
fn private_network_requires_explicit_option() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let dir = std::env::temp_dir().join(format!("ue-private-network-{}", std::process::id()));
    let result = Command::new(env!("CARGO_BIN_EXE_ue-weave"))
        .arg(format!("http://{}", listener.local_addr().unwrap()))
        .arg("--output")
        .arg(&dir)
        .arg("--json")
        .output()
        .unwrap();
    assert!(!result.status.success());
    let error: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(error["code"], "E0109");
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
}

fn read_request(stream: &mut std::net::TcpStream) -> String {
    // Accepted sockets can inherit the listener's nonblocking mode.
    stream.set_nonblocking(false).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut bytes = Vec::new();
    while !bytes.windows(4).any(|window| window == b"\r\n\r\n") {
        let mut chunk = [0; 1024];
        let length = stream.read(&mut chunk).unwrap();
        assert!(length > 0, "连接在请求头读取完成前关闭");
        bytes.extend_from_slice(&chunk[..length]);
        assert!(bytes.len() <= 8192, "请求头超过测试限制");
    }
    String::from_utf8(bytes).unwrap()
}

fn serve(responses: Vec<(u16, String)>) -> (String, thread::JoinHandle<usize>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = thread::spawn(move || {
        let mut count = 0;
        for (status, body) in responses {
            let deadline = Instant::now() + Duration::from_secs(5);
            let mut stream = loop {
                if let Ok((stream, _)) = listener.accept() {
                    break stream;
                }
                if Instant::now() > deadline {
                    return count;
                }
                thread::sleep(Duration::from_millis(5));
            };
            let _ = read_request(&mut stream);
            write!(
                stream,
                "HTTP/1.1 {status} Test\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
            count += 1;
        }
        count
    });
    (url, server)
}

#[test]
fn page_and_attachment_retry_and_local_config_use_same_pipeline() {
    let page = "<h1>本地站点标题</h1><section class='body'><p><span class='wp_pdf_player' pdfsrc='/file.pdf' \
        sudyfile-attr=\"{'title':'通知.pdf'}\"></span></p></section>";
    let (url, server) = serve(vec![
        (503, "".into()),
        (200, page.into()),
        (503, "".into()),
        (200, "PDF test bytes".into()),
    ]);
    let dir = std::env::temp_dir().join(format!("ue-http-cli-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let config = dir.join("sites.json");
    fs::write(&config, r#"{"content":".body","title":"h1"}"#).unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_ue-weave"))
        .arg(&url)
        .args(["--formats", "html,markdown,typst,json"])
        .arg("--allow-private-network")
        .args([
            "--download-attachments",
            "--retries",
            "1",
            "--timeout",
            "2",
            "--json",
            "--verbose",
            "--site-config",
        ])
        .arg(&config)
        .arg("--output")
        .arg(dir.join("out"))
        .env("NO_PROXY", "127.0.0.1")
        .env("no_proxy", "127.0.0.1")
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(server.join().unwrap(), 4);
    let report: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(report["selector"], ".body");
    assert!(report["attachment_errors"].as_array().unwrap().is_empty());
    assert_eq!(
        String::from_utf8_lossy(&result.stderr)
            .matches("| retry:")
            .count(),
        2
    );
    let ir: serde_json::Value =
        serde_json::from_slice(&fs::read(dir.join("out/reconstructed.json")).unwrap()).unwrap();
    assert_eq!(ir["metadata"]["title"], "本地站点标题");
    let local = ir["attachments"][0]["href"].as_str().unwrap();
    assert_eq!(
        fs::read(dir.join("out").join(local)).unwrap(),
        b"PDF test bytes"
    );
    for file in [
        "reconstructed.html",
        "reconstructed.md",
        "reconstructed.typ",
    ] {
        assert!(fs::read_to_string(dir.join("out").join(file))
            .unwrap()
            .contains(local));
    }
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn exhausted_retries_produce_structured_error() {
    let (url, server) = serve(vec![(503, "".into()), (503, "".into())]);
    let dir = std::env::temp_dir().join(format!("ue-http-error-{}", std::process::id()));
    let result = Command::new(env!("CARGO_BIN_EXE_ue-weave"))
        .arg(&url)
        .args([
            "--allow-private-network",
            "--retries",
            "1",
            "--timeout",
            "2",
            "--json",
            "--output",
        ])
        .arg(&dir)
        .env("NO_PROXY", "127.0.0.1")
        .env("no_proxy", "127.0.0.1")
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert_eq!(server.join().unwrap(), 2);
    let error: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(error["code"], "E0106");
    assert_eq!(error["network"]["http_status"], 503);
    assert_eq!(error["network"]["attempts"], 2);
    assert_eq!(error["network"]["url"], format!("{url}/"));
    assert!(!dir.join("reconstructed.html").exists());
}

#[test]
fn custom_headers_are_sent_to_the_page_request() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = String::new();
        let mut buffer = [0u8; 4096];
        let size = stream.read(&mut buffer).unwrap();
        request.push_str(&String::from_utf8_lossy(&buffer[..size]));
        assert!(
            request
                .to_ascii_lowercase()
                .contains("referer: https://example.edu/"),
            "{request}"
        );
        assert!(request.contains("x-test: accepted"), "{request}");
        let body = "<div class='body'><p>正文</p></div>";
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
    });
    let dir = std::env::temp_dir().join(format!("ue-http-header-{}", std::process::id()));
    let output = Command::new(env!("CARGO_BIN_EXE_ue-weave"))
        .arg(&url)
        .args([
            "--allow-private-network",
            "--header",
            "Referer=https://example.edu/",
        ])
        .args([
            "--header",
            "X-Test=accepted",
            "--selector",
            ".body",
            "--output",
        ])
        .arg(&dir)
        .env("NO_PROXY", "127.0.0.1")
        .env("no_proxy", "127.0.0.1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    server.join().unwrap();
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn invalid_header_configuration_has_dedicated_error_code() {
    let output = Command::new(env!("CARGO_BIN_EXE_ue-weave"))
        .args([
            "http://example.org/",
            "--header",
            "Host=bad",
            "--output",
            "/tmp/ue-invalid-header",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("E0014"), "{stderr}");
}

#[test]
fn headers_file_errors_use_dedicated_error_code() {
    let file = std::env::temp_dir().join(format!("ue-invalid-headers-{}.json", std::process::id()));
    fs::write(&file, "[]").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_ue-weave"))
        .args(["http://example.org/", "--headers-file"])
        .arg(&file)
        .args(["--output", "/tmp/ue-invalid-headers-file"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("E0003"));
    fs::remove_file(file).unwrap();
}

#[test]
fn header_file_examples_match_explained_codes() {
    let dir = tempfile::tempdir().unwrap();
    for (name, content, code) in [
        ("missing.json", None, "E0002"),
        ("broken.json", Some("{"), "E0003"),
        ("number.json", Some(r#"{"X-Test": 12}"#), "E0003"),
        ("managed.json", Some(r#"{"Host": "example.org"}"#), "E0014"),
    ] {
        let file = dir.path().join(name);
        if let Some(content) = content {
            fs::write(&file, content).unwrap();
        }
        let output = Command::new(env!("CARGO_BIN_EXE_ue-weave"))
            .args(["http://example.org/", "--headers-file"])
            .arg(file)
            .arg("--output")
            .arg(dir.path().join("out"))
            .output()
            .unwrap();
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains(code), "{name}: {stderr}");
    }
}
