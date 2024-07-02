use std::{collections::HashMap, path::Path, sync::Arc};

use anyhow::anyhow;
use base_io_traits::fs_traits::FileSystemInterface;
use cache::Cache;
use hiarc::Hiarc;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct CompileDefinitionFile {
    input: String,
    output: String,
    #[serde(default)]
    defines: HashMap<String, bool>,
}

#[derive(Debug, Hiarc, Clone, Copy)]
pub enum ShaderCompilerType {
    // this is mostly implemented to check the output in human readable format
    #[cfg(test)]
    WgslInGlslOut,
    WgslInSpvOut,
}

#[derive(Debug, Hiarc)]
pub struct ShaderCompiler {
    pub(crate) ty: ShaderCompilerType,
    #[hiarc_skip_unsafe]
    pub(crate) fs: Arc<dyn FileSystemInterface>,

    cache: Arc<Cache<20231227>>,

    pub(crate) shader_files: HashMap<String, Vec<u32>>,
}

impl ShaderCompiler {
    pub fn new(ty: ShaderCompilerType, fs: Arc<dyn FileSystemInterface>) -> Self {
        Self {
            ty,
            cache: Arc::new(Cache::new("wgsl", &fs)),
            fs,
            shader_files: Default::default(),
        }
    }

    pub fn new_with_files(
        ty: ShaderCompilerType,
        fs: Arc<dyn FileSystemInterface>,
        shader_files: HashMap<String, Vec<u32>>,
    ) -> Self {
        Self {
            ty,
            cache: Arc::new(Cache::new("wgsl", &fs)),
            fs,
            shader_files,
        }
    }

    fn shader_module_with_preprocessor(
        name: &str,
        source: &str,
        defines: HashMap<String, bool>,
    ) -> anyhow::Result<naga::Module> {
        let mut composer = naga_oil::compose::Composer::default().with_capabilities(
            naga::valid::Capabilities::PUSH_CONSTANT,
            naga::valid::ShaderStages::FRAGMENT | naga::valid::ShaderStages::VERTEX,
        );
        Ok(
            composer.make_naga_module(naga_oil::compose::NagaModuleDescriptor {
                source,
                file_path: name,
                shader_type: naga_oil::compose::ShaderType::Wgsl,
                shader_defs: defines
                    .into_iter()
                    .map(|(name, val)| (name, naga_oil::compose::ShaderDefValue::Bool(val)))
                    .collect(),
                additional_imports: &[],
            })?,
        )
    }

    fn compile_spv(module: naga::Module) -> anyhow::Result<Vec<u32>> {
        use naga::back::spv;
        Ok(spv::write_vec(
            &module,
            &naga::valid::Validator::new(
                naga::valid::ValidationFlags::empty(),
                naga::valid::Capabilities::PUSH_CONSTANT,
            )
            .validate(&module)?,
            &spv::Options {
                flags: spv::WriterFlags::empty(),
                ..Default::default()
            },
            None,
        )?)
    }

    #[cfg(test)]
    fn compile_glsl(module: naga::Module, is_fragment: bool) -> anyhow::Result<Vec<u32>> {
        use naga::back::glsl;

        let pipeline_options = glsl::PipelineOptions {
            shader_stage: if is_fragment {
                naga::ShaderStage::Fragment
            } else {
                naga::ShaderStage::Vertex
            },
            entry_point: "main".to_string(),
            multiview: None,
        };

        let info = naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        )
        .validate(&module)?;

        let mut options = glsl::Options::default();
        options.version = glsl::Version::Desktop(330);

        let mut buffer = String::new();
        let mut writer = glsl::Writer::new(
            &mut buffer,
            &module,
            &info,
            &options,
            &pipeline_options,
            naga::proc::BoundsCheckPolicies::default(),
        )?;
        writer.write()?;
        Ok(buffer.chars().map(|char| char as u32).collect())
    }

    /// returns a Vec<u32>:
    /// - in case of GLSL this is the unicode point representation of all chars
    /// - in case of SPIR-V this is the binary representation using little endian byte order (required by the standard)
    pub async fn compile(&mut self, path: &Path, compile_json_file: &Path) -> anyhow::Result<()> {
        let json_file = self.fs.read_file(&path.join(compile_json_file)).await?;
        let files_to_compile: Vec<CompileDefinitionFile> = serde_json::from_slice(&json_file)?;

        for file_res in
            futures::future::join_all(files_to_compile.into_iter().map(|file_to_compile| {
                let fs = self.fs.clone();
                let ty = self.ty;
                let cache = self.cache.clone();
                async move {
                    let shader_source = fs.read_file(&path.join(&file_to_compile.input)).await?;

                    let mut defines_hashable: Vec<(String, bool)> =
                        file_to_compile.defines.clone().into_iter().collect();
                    defines_hashable.sort_by(|(v1, _), (v2, _)| v1.cmp(v2));
                    let defines_hashable: Vec<u8> = defines_hashable
                        .into_iter()
                        .flat_map(|(name, val)| {
                            let mut res = name.as_bytes().to_vec();
                            res.push(val as u8);
                            res
                        })
                        .collect();

                    let shader_from_cache = cache
                        .load_from_binary_ex(&shader_source, &defines_hashable, |file| {
                            let module = Self::shader_module_with_preprocessor(
                                &file_to_compile.input,
                                &String::from_utf8(file.to_vec()).unwrap(),
                                file_to_compile.defines,
                            )
                            .map_err(|err| {
                                anyhow!(
                                    "failed to create module for: {} ({err})",
                                    file_to_compile.input
                                )
                            })?;

                            let shader_file = match ty {
                                #[cfg(test)]
                                ShaderCompilerType::WgslInGlslOut => Self::compile_glsl(
                                    module,
                                    file_to_compile.input.contains(".frag"),
                                ),
                                ShaderCompilerType::WgslInSpvOut => Self::compile_spv(module),
                            }
                            .map_err(|err| {
                                anyhow!(
                                    "failed to compile module for: {} ({err})",
                                    file_to_compile.input
                                )
                            })?;

                            // SPIR-V wants little endian
                            Ok(shader_file
                                .into_iter()
                                .flat_map(|val| val.to_le_bytes())
                                .collect())
                        })
                        .await?;

                    let shader_file = shader_from_cache
                        .chunks_exact(std::mem::size_of::<u32>())
                        .map(|val| {
                            let val = [val[0], val[1], val[2], val[3]];
                            u32::from_le_bytes(val)
                        })
                        .collect();

                    Ok::<(String, Vec<u32>), anyhow::Error>((file_to_compile.output, shader_file))
                }
            }))
            .await
        {
            let (name, shader_file) = file_res?;
            self.shader_files.insert(name, shader_file);
        }

        Ok(())
    }
}
