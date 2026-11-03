//! Re-render an editable IR bundle without fetching or executing remote content.
use crate::{
    error::{RecoverError, Result},
    model::Ir,
};
use std::{
    fs,
    path::{Component, Path},
};

fn read(path: &Path) -> Result<Vec<u8>> {
    fs::read(path).map_err(|source| RecoverError::ReadInput {
        path: path.into(),
        source,
    })
}

fn write(path: &Path, bytes: &[u8]) -> Result<()> {
    fs::write(path, bytes).map_err(|source| RecoverError::WriteOutput {
        path: path.into(),
        source,
    })
}

pub fn run(
    path: &Path,
    output: &Path,
    json: bool,
    verbose: bool,
    profile: crate::profile::Profile,
    settings: crate::profile::Settings,
) -> Result<()> {
    let logger = crate::log::Logger::new(verbose);
    logger.stage("input-ir", format!("读取 {}", path.display()));
    let mut ir: Ir =
        serde_json::from_slice(&read(path)?).map_err(|source| RecoverError::ReadIr {
            path: path.into(),
            source,
        })?;
    let directory = path.parent().unwrap_or(Path::new("."));
    // Read required provenance before writing any output.
    let original = read(&directory.join("original.html"))?;
    let mut files = vec![("original.html".to_string(), original)];
    for name in ["source.bin", "source.html"] {
        if directory.join(name).is_file() {
            files.push((name.into(), read(&directory.join(name))?));
        }
    }
    let canonical_directory =
        fs::canonicalize(directory).map_err(|source| RecoverError::ReadInput {
            path: directory.into(),
            source,
        })?;
    for asset in ir.attachments.iter_mut().chain(&mut ir.images) {
        let Some(local) = asset.get("local_path").and_then(|v| v.as_str()) else {
            continue;
        };
        let local = local.to_string();
        let original_path = Path::new(&local);
        let relative = if original_path.is_absolute() {
            original_path
                .strip_prefix(&canonical_directory)
                .map_err(|_| RecoverError::Arguments(format!("IR 资源指向输出包外部：{local}")))?
        } else {
            original_path
        };
        if relative
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
            || !matches!(relative.components().next(), Some(Component::Normal(name)) if name == "attachments" || name == "images")
        {
            return Err(RecoverError::Arguments(format!(
                "IR 资源路径必须位于 attachments/ 或 images/：{local}"
            )));
        }
        let source_path = directory.join(relative);
        let canonical =
            fs::canonicalize(&source_path).map_err(|source| RecoverError::ReadInput {
                path: source_path.clone(),
                source,
            })?;
        if !canonical.starts_with(&canonical_directory) {
            return Err(RecoverError::Arguments(format!(
                "IR 资源指向输出包外部：{local}"
            )));
        }
        let relative = relative.to_string_lossy().into_owned();
        files.push((relative.clone(), read(&canonical)?));
        asset["local_path"] = relative.into();
    }
    let report = serde_json::json!({"mode":"regenerate","source":ir.source,"warnings":ir.warnings,"counts":{"blocks":ir.blocks.len(),"total_blocks":ir.blocks.iter().map(crate::model::Block::block_count).sum::<usize>(),"attachments":ir.attachments.len(),"images":ir.images.len()},"note":"使用编辑后的 IR 重建；原始快照保持原样，原还原警告作为来源记录保留"});
    let rendered = crate::profile::prepare(&ir, settings);
    let mut report = report;
    report["output_profile"] = serde_json::json!(profile);
    report["output_options"] = serde_json::json!(settings);
    report["output_changes"] = serde_json::json!(rendered.changes);
    for (name, value) in [
        (
            "reconstructed.json",
            serde_json::to_value(&ir).map_err(|source| RecoverError::Serialize {
                file: "reconstructed.json".into(),
                source,
            })?,
        ),
        ("report.json", report.clone()),
    ] {
        files.push((
            name.into(),
            serde_json::to_vec_pretty(&value).map_err(|source| RecoverError::Serialize {
                file: name.into(),
                source,
            })?,
        ));
    }
    files.extend([
        (
            "reconstructed.html".into(),
            crate::renderer::render(&rendered.ir).into_bytes(),
        ),
        (
            "reconstructed.md".into(),
            if settings.native_markdown {
                crate::markdown::render_editable(&rendered.ir)
            } else {
                crate::markdown::render(&rendered.ir)
            }
            .into_bytes(),
        ),
        (
            "reconstructed.typ".into(),
            crate::typst::render(&rendered.ir).into_bytes(),
        ),
    ]);
    fs::create_dir_all(output).map_err(|source| RecoverError::CreateOutput {
        path: output.into(),
        source,
    })?;
    for (name, bytes) in files {
        let target = output.join(name);
        let parent = target.parent().expect("output file parent");
        fs::create_dir_all(parent).map_err(|source| RecoverError::CreateOutput {
            path: parent.into(),
            source,
        })?;
        write(&target, &bytes)?;
    }
    logger.warnings(&ir.warnings);
    logger.done(
        ir.blocks.iter().map(crate::model::Block::block_count).sum(),
        ir.attachments.len(),
        ir.images.len(),
    );
    if json {
        println!("{report}");
    }
    Ok(())
}
