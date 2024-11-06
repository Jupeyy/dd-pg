pub mod backend {
    use std::{rc::Rc, sync::Arc};

    use graphics_backend_traits::{
        plugin::GraphicsObjectRewriteFunc, traits::GraphicsBackendInterface, types::BackendCommands,
    };
    use graphics_base_traits::traits::GraphicsStreamedData;
    use graphics_types::{
        commands::{
            AllCommands, CommandClear, CommandIndicesForQuadsRequiredNotify, CommandsMisc,
            CommandsRender,
        },
        gpu::Gpus,
        rendering::ColorRgba,
        types::{GraphicsBackendMemory, GraphicsMemoryAllocationType},
    };
    use hiarc::Hiarc;
    use pool::{mixed_pool::PoolSyncPoint, mt_datatypes::PoolVec};

    #[derive(Debug, Hiarc)]
    pub struct GraphicsBackendHandle {
        pub backend_cmds: BackendCommands,
        #[hiarc_skip_unsafe]
        pub(crate) backend: Rc<dyn GraphicsBackendInterface>,
    }

    impl Clone for GraphicsBackendHandle {
        fn clone(&self) -> Self {
            Self {
                backend_cmds: self.backend_cmds.clone(),
                backend: self.backend.clone(),
            }
        }
    }

    impl GraphicsBackendHandle {
        pub fn run_backend_buffer(&self, stream_data: &GraphicsStreamedData) {
            self.backend.run_cmds(&self.backend_cmds, stream_data);
        }

        pub fn add_cmd(&self, cmd: AllCommands) {
            self.backend_cmds.add_cmd(cmd);
        }

        pub fn mem_alloc(&self, alloc_type: GraphicsMemoryAllocationType) -> GraphicsBackendMemory {
            self.backend.mem_alloc(alloc_type)
        }

        /// Switching to a rendering pass that supports
        /// stencil buffers and color attachments as input
        /// for rendering operations.  
        /// __It does not support multi sampling. Additionally
        /// it automatically consumes multi sampling targets just like
        /// [GraphicsBackendHandle::consumble_multi_samples]__
        pub fn next_switch_pass(&self) {
            self.add_cmd(AllCommands::Misc(CommandsMisc::NextSwitchPass));
        }

        /// __Once__ per frame, the implementation can consume the multi sample
        /// targets, resolving them into a single color attachment target.
        /// You can even call this function (and in fact simply should) if
        /// multi sampling is not active, the backend decides.
        /// Calling this function more than __once__ results in silently ignoring it,
        /// it will not panic!
        /// Switching back to multi samples after this function was called,
        /// is impossible  
        /// (If for whatever reason you need multiple multi sampling
        /// renders per frame, see the offscreen canvas support in
        /// [`super::super::canvas::canvas::GraphicsCanvasHandle::switch_canvas`])
        pub fn consumble_multi_samples(&self) {
            self.add_cmd(AllCommands::Misc(CommandsMisc::ConsumeMultiSamplingTargets));
        }

        /// Updates the clear color of the backend
        pub fn update_clear_color(&self, clear_color: ColorRgba) {
            self.add_cmd(AllCommands::Render(CommandsRender::Clear(CommandClear {
                color: clear_color,
                force_clear: false,
            })));
        }

        pub fn indices_for_quads_required_notify(&self, quad_count_required: u64) {
            let cmd = CommandIndicesForQuadsRequiredNotify {
                quad_count_required,
            };

            self.add_cmd(AllCommands::Misc(
                CommandsMisc::IndicesForQuadsRequiredNotify(cmd),
            ));
        }

        pub fn check_mod_cmd(
            &self,
            mod_name: &str,
            cmd: &mut PoolVec<u8>,
            f: &dyn Fn(GraphicsObjectRewriteFunc),
        ) {
            self.backend.check_mod_cmd(mod_name, cmd, f)
        }

        pub fn add_sync_point(&self, sync_point: Box<dyn PoolSyncPoint>) {
            self.backend.add_sync_point(sync_point)
        }

        pub fn gpus(&self) -> Arc<Gpus> {
            self.backend.gpus()
        }
    }

    impl GraphicsBackendHandle {
        pub fn new(backend: Rc<dyn GraphicsBackendInterface>) -> Self {
            Self {
                backend_cmds: BackendCommands::default(),
                backend,
            }
        }
    }
}
