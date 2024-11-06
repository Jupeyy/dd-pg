use std::{
    collections::HashMap,
    net::SocketAddr,
    path::{Path, PathBuf},
};

use anyhow::anyhow;
use assets_base::AssetsIndex;
use axum::Router;
use base::hash::{fmt_hash, generate_hash_for};
use clap::{command, Parser};
use path_slash::PathBufExt;
use tar::Header;
use tower_http::{services::ServeDir, trace::TraceLayer};

#[derive(clap::ValueEnum, Debug, Clone, Copy)]
enum ImportType {
    Skins,
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// The port this server should listen on
    #[arg(short, long)]
    port: Option<u16>,
    /// Path to files to import (generate hashes for those files and create an index.json).
    /// Must be used together with import_type.
    #[arg(short, long)]
    import: Option<PathBuf>,
    /// The type of files that are about to be imported.
    /// Must be used together with import.
    #[arg(short, long)]
    import_type: Option<ImportType>,
    /// Don't start the server itself.
    #[arg(short, long)]
    dry: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    // imports
    if args.import.is_some() || args.import_type.is_some() {
        let import = args
            .import
            .ok_or_else(|| anyhow!("import path not specified."))?;
        let import_type = args
            .import_type
            .ok_or_else(|| anyhow!("import_type path not specified or invalid."))?;

        match import_type {
            ImportType::Skins => {
                import_generic(&import, "skins").await?;
            }
        }
    }

    if args.dry {
        log::info!("Exiting because dry run was requested.");
        return Ok(());
    }

    let port = args.port.unwrap_or(3002);
    let addr: SocketAddr = format!("127.0.0.1:{port}").parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let app = skins().await?;
    axum::serve(listener, app.layer(TraceLayer::new_for_http())).await?;

    Ok(())
}

async fn assets_generic(base_path: &str) -> anyhow::Result<Router> {
    // make sure there is an index file for this path
    let index_path = format!("{base_path}/index.json");
    if !tokio::fs::try_exists(&index_path).await.unwrap_or_default() {
        let index = prepare_index_generic(base_path).await?;

        tokio::fs::write(index_path, serde_json::to_vec(&index)?).await?;
    }

    Ok(Router::new().nest_service(&format!("/{base_path}"), ServeDir::new(base_path)))
}

async fn skins() -> anyhow::Result<Router> {
    assets_generic("skins").await
}

async fn prepare_index_generic(base_path: &str) -> anyhow::Result<AssetsIndex> {
    let mut res: AssetsIndex = Default::default();

    let mut files = tokio::fs::read_dir(base_path)
        .await
        .map_err(|err| anyhow!("can't dir find {base_path:?}: {err}"))?;

    while let Some(file) = files.next_entry().await? {
        anyhow::ensure!(
            file.metadata().await?.is_file(),
            "only files are allowed as assets files currently."
        );
        let path = file.path();

        let file_name = path
            .file_stem()
            .ok_or_else(|| anyhow!("Only files with proper names are allowed"))?
            .to_string_lossy()
            .to_string();
        let file_ext = path
            .extension()
            .ok_or_else(|| anyhow!("Files need proper file endings"))?
            .to_string_lossy()
            .to_string();

        let file = tokio::fs::read(&path)
            .await
            .map_err(|err| anyhow!("can't find {path:?}: {err}"))?;

        let hash = generate_hash_for(&file);

        anyhow::ensure!(
            file_name.ends_with(&format!("_{}", fmt_hash(&hash))),
            "Only files with their hashes are allowed.\
            To import files without hashes use the --import arg."
        );

        res.insert(
            file_name,
            assets_base::AssetIndexEntry { ty: file_ext, hash },
        );
    }

    Ok(res)
}

async fn import_generic(import_path: &Path, base_path: &str) -> anyhow::Result<()> {
    let mut index: AssetsIndex = Default::default();

    tokio::fs::create_dir_all(base_path).await?;

    let mut files = tokio::fs::read_dir(import_path)
        .await
        .map_err(|err| anyhow!("can't dir find {import_path:?}: {err}"))?;

    while let Some(entry) = files.next_entry().await? {
        let meta_data = entry.metadata().await?;
        anyhow::ensure!(
            meta_data.is_file() || meta_data.is_dir(),
            "only files and dirs are allowed for import currently."
        );
        let path = entry.path();

        if meta_data.is_file() {
            let file_name = path
                .file_stem()
                .ok_or_else(|| anyhow!("Only files with proper names are allowed"))?
                .to_string_lossy()
                .to_string();
            let file_ext = path
                .extension()
                .ok_or_else(|| anyhow!("Files need proper file endings"))?
                .to_string_lossy()
                .to_string();

            let file = tokio::fs::read(&path)
                .await
                .map_err(|err| anyhow!("can't find {path:?}: {err}"))?;

            let hash = generate_hash_for(&file);
            let hash_str = format!("_{}", fmt_hash(&hash));

            let file_name = if let Some(pos) = file_name
                .ends_with(&hash_str)
                .then(|| file_name.rfind(&hash_str))
                .flatten()
            {
                let (before, _) = file_name.split_at(pos);
                before.to_string()
            } else {
                file_name
            };

            tokio::fs::write(
                format!("{base_path}/{}{}.{}", file_name, hash_str, file_ext),
                file,
            )
            .await?;

            index.insert(
                file_name,
                assets_base::AssetIndexEntry { ty: file_ext, hash },
            );
        } else if meta_data.is_dir() {
            async fn files_in_dir_recursive(
                path: &Path,
                rec_path: PathBuf,
            ) -> anyhow::Result<HashMap<PathBuf, Vec<u8>>> {
                let mut read_dirs = vec![rec_path.clone()];
                let mut file_list: HashMap<PathBuf, Vec<u8>> = Default::default();

                while let Some(rec_path) = read_dirs.pop() {
                    let path = path.join(&rec_path);
                    let mut dir_reader = tokio::fs::read_dir(&path)
                        .await
                        .map_err(|err| anyhow!("can't dir find {path:?}: {err}"))?;

                    while let Some(entry) = dir_reader.next_entry().await? {
                        let meta_data = entry.metadata().await?;

                        let entry_name = entry.file_name();
                        let file_path = rec_path.join(&entry_name);

                        if meta_data.is_file() && !file_list.contains_key(&file_path) {
                            let file = tokio::fs::read(entry.path())
                                .await
                                .map_err(|err| anyhow!("can't find {entry_name:?}: {err}"))?;
                            let file_path_slash = file_path.to_slash_lossy().as_ref().into();
                            file_list.insert(file_path_slash, file);
                        } else if meta_data.is_dir() {
                            read_dirs.push(file_path);
                        }
                    }
                }

                Ok(file_list)
            }

            let files = files_in_dir_recursive(&path, "".into()).await?;
            let mut files: Vec<_> = files.into_iter().collect();
            files.sort_by_key(|(f, _)| f.as_os_str().to_string_lossy().to_string());

            let mut builder = tar::Builder::new(Vec::new());
            builder.mode(tar::HeaderMode::Deterministic);

            for (name, file) in files {
                let mut header = Header::new_gnu();
                header.set_cksum();
                header.set_size(file.len() as u64);
                header.set_mode(0o644);
                header.set_uid(1000);
                header.set_gid(1000);
                builder.append_data(&mut header, name, std::io::Cursor::new(&file))?;
            }

            let entry_name = entry.file_name().to_string_lossy().to_string();
            let tar_file = builder.into_inner()?;
            let hash = generate_hash_for(&tar_file);
            let hash_str = format!("_{}", fmt_hash(&hash));

            let entry_name = if let Some(pos) = entry_name
                .ends_with(&hash_str)
                .then(|| entry_name.rfind(&hash_str))
                .flatten()
            {
                let (before, _) = entry_name.split_at(pos);
                before.to_string()
            } else {
                entry_name
            };

            tokio::fs::write(
                format!("{base_path}/{}{}.tar", entry_name, hash_str),
                tar_file,
            )
            .await?;

            index.insert(
                entry_name,
                assets_base::AssetIndexEntry {
                    ty: "tar".to_string(),
                    hash,
                },
            );
        }
    }

    tokio::fs::write(
        format!("{base_path}/index.json"),
        serde_json::to_vec(&index)?,
    )
    .await?;

    Ok(())
}
