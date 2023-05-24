use std::sync::{Arc, Mutex};

use ash::vk;

use super::common::{EGFXErrorType, EGFXWarningType, SGFXErrorContainer, SGFXWarningContainer};

pub struct Error {
    /************************
     * ERROR MANAGEMENT
     ************************/
    pub has_error: bool,
    pub can_assert: bool,

    pub error: SGFXErrorContainer,
    pub warning: SGFXWarningContainer,
}

impl Default for Error {
    fn default() -> Self {
        Self {
            has_error: Default::default(),
            can_assert: Default::default(),
            error: Default::default(),
            warning: Default::default(),
        }
    }
}

impl Error {
    /************************
     * Error handling
     ************************/
    /**
     * After an error occured, the rendering stop as soon as possible
     * Always stop the current code execution after a call to this function (e.g.
     * return false)
     */
    pub fn SetErrorExtra(
        &mut self,
        ErrType: EGFXErrorType,
        pErr: &str,
        pErrStrExtra: Option<&str>,
    ) {
        if self.error.errors.contains(&pErr.to_string()) {
            self.error.errors.push(pErr.to_string());
        }
        if let Some(err_extra) = pErrStrExtra {
            if self.error.errors.contains(&err_extra.to_string()) {
                self.error.errors.push(err_extra.to_string());
            }
        }
        if self.can_assert {
            /* TODO: if(pErrStrExtra != std::ptr::null())
                dbg_msg("vulkan", "vulkan error: %s: %s", pErr, pErrStrExtra);
            else
                dbg_msg("vulkan", "vulkan error: %s", pErr);*/
            self.has_error = true;
            self.error.error_type = ErrType;
        } else {
            // during initialization vulkan should not throw any errors but warnings
            // instead since most code in the swapchain is shared with runtime code,
            // add this extra code path
            self.SetWarning(EGFXWarningType::InitFailed, pErr);
        }
    }

    pub fn SetError(&mut self, ErrType: EGFXErrorType, pErr: &str) {
        self.SetErrorExtra(ErrType, pErr, None);
    }

    pub fn SetWarningPreMsg(&mut self, pWarningPre: &str) {
        if self.warning.warnings.contains(&pWarningPre.to_string()) {
            self.warning.warnings.push_front(pWarningPre.to_string());
        }
    }

    pub fn SetWarning(&mut self, WarningType: EGFXWarningType, pWarning: &str) {
        // TODO dbg_msg("vulkan", "vulkan warning: %s", pWarning);
        if self.warning.warnings.contains(&pWarning.to_string()) {
            self.warning.warnings.push_back(pWarning.to_string());
        }
        self.warning.warning_type = WarningType;
    }
}

#[derive(Default)]
pub struct CheckResult {
    error_helper: String,
}

impl CheckResult {
    // TODO: using options for errors is weird
    pub fn CheckVulkanCriticalError(
        &mut self,
        CallResult: vk::Result,
        error: &Arc<Mutex<Error>>,
        recreate_swap_chain: &mut bool,
    ) -> Option<&'static str> {
        let mut critical_error = None;
        match CallResult {
            vk::Result::ERROR_OUT_OF_HOST_MEMORY => {
                critical_error = Some("host ran out of memory");
                // TODO dbg_msg("vulkan", "%s", pCriticalError);
            }
            vk::Result::ERROR_OUT_OF_DEVICE_MEMORY => {
                critical_error = Some("device ran out of memory");
                // TODO dbg_msg("vulkan", "%s", pCriticalError);
            }
            vk::Result::ERROR_DEVICE_LOST => {
                critical_error = Some("device lost");
                // TODO dbg_msg("vulkan", "%s", pCriticalError);
            }
            vk::Result::ERROR_OUT_OF_DATE_KHR => {
                {
                    // TODO if(IsVerbose(&*self.dbg))
                    // TODO {
                    // TODO     dbg_msg("vulkan", "queueing swap chain recreation because the current "
                    // TODO               "is out of date");
                    // TODO }
                    *recreate_swap_chain = true;
                }
            }
            vk::Result::ERROR_SURFACE_LOST_KHR => {
                // TODO dbg_msg("vulkan", "surface lost");
            }
            /*vk::Result::ERROR_FULL_SCREEN_EXCLUSIVE_MODE_LOST_EXT=> {            dbg_msg("vulkan", "fullscreen exclusive mode lost");
            break;*/
            vk::Result::ERROR_INCOMPATIBLE_DRIVER => {
                critical_error = Some("no compatible driver found. Vulkan 1.1 is required.");
                // TODO  dbg_msg("vulkan", "%s", pCriticalError);
            }
            vk::Result::ERROR_INITIALIZATION_FAILED => {
                critical_error = Some("initialization failed for unknown reason.");
                // TODO  dbg_msg("vulkan", "%s", pCriticalError);
            }
            vk::Result::ERROR_LAYER_NOT_PRESENT => {
                error.lock().unwrap().SetWarning(
                    EGFXWarningType::MissingExtension,
                    "One Vulkan layer was not present. (try to disable them)",
                );
            }
            vk::Result::ERROR_EXTENSION_NOT_PRESENT => {
                error.lock().unwrap().SetWarning(
                    EGFXWarningType::MissingExtension,
                    "One Vulkan extension was not present. (try to disable them)",
                );
            }
            vk::Result::ERROR_NATIVE_WINDOW_IN_USE_KHR => {
                // TODO dbg_msg("vulkan", "native window in use");
            }
            vk::Result::SUCCESS => {}
            vk::Result::SUBOPTIMAL_KHR => {
                // TODO if(IsVerbose(&*self.dbg))
                // TODO {
                // TODO     dbg_msg("vulkan", "queueing swap chain recreation because the current "
                // TODO               "is sub optimal");
                // TODO }
                *recreate_swap_chain = true;
            }
            _ => {
                // TODO self.m_ErrorHelper = ("unknown error");
                // TODO self.m_ErrorHelper.append(format!("{}", (CallResult)));
                // TODO pCriticalError = self.m_ErrorHelper.c_str();
            }
        }

        return critical_error;
    }
}
