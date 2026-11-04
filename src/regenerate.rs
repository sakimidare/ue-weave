//! 读取已编辑的文章 JSON 和本地资源，重新生成文档。
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

pub fn run(path: &Path, args: &crate::cli::Args, output: &Path) -> Result<()> {
    let json = args.json;
    let verbose = args.verbose;
    let profile = args.profile;
    let settings = args.output_options.resolve(profile);
    let asset_root = args.asset_root.as_deref();
    let exports = &args.exports;
    let logger = crate::log::Logger::with_color(verbose, args.color);
    logger.stage("input-ir", format!("读取 {}", path.display()));
    let mut ir: Ir = crate::ir_format::read(&read(path)?, path)?;
    let directory = path.parent().unwrap_or(Path::new("."));
    let mut files = Vec::new();
    let resource_directory = asset_root.unwrap_or(directory);
    let canonical_directory =
        fs::canonicalize(resource_directory).map_err(|source| RecoverError::ReadInput {
            path: resource_directory.into(),
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
                "IR 资源路径超出 attachments/ 和 images/：{local}"
            )));
        }
        let source_path = resource_directory.join(relative);
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
        let bytes = read(&canonical)?;
        files.push((relative.clone(), bytes));
        asset["local_path"] = relative.into();
    }
    let rendered = crate::profile::prepare_documents(&ir, settings);
    let documents = crate::export::render(&rendered.ir, settings, exports);
    files.extend(exports.files(documents, &ir)?);
    fs::create_dir_all(output).map_err(|source| RecoverError::CreateOutput {
        path: output.into(),
        source,
    })?;
    for (name, bytes) in files {
        let target = output.join(&name);
        let parent = target.parent().expect("output file parent");
        fs::create_dir_all(parent).map_err(|source| RecoverError::CreateOutput {
            path: parent.into(),
            source,
        })?;
        write(&target, &bytes)?;
    }
    logger.warnings(&ir.warnings, &ir.original_nodes, &rendered.ir);
    logger.done(
        ir.blocks.iter().map(crate::model::Block::block_count).sum(),
        ir.attachments.len(),
        ir.images.len(),
    );
    if json {
        println!(
            "{}",
            serde_json::json!({
                "status": "ok",
                "source": ir.source,
                "warnings": ir.warnings,
                "output_profile": profile,
                "output_options": settings,
                "counts": {"blocks":ir.blocks.len(),"attachments":ir.attachments.len(),"images":ir.images.len()},
            })
        );
    }
    logger.finish_diagnostics(&ir.warnings);
    Ok(())
}
