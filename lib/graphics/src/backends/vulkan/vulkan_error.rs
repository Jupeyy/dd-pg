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
    pub fn set_error_extra(
        &mut self,
        err_type: EGFXErrorType,
        err_str: &str,
        err_str_extra: Option<&str>,
    ) {
        if self.error.errors.contains(&err_str.to_string()) {
            self.error.errors.push(err_str.to_string());
        }
        if let Some(err_extra) = err_str_extra {
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
            self.error.error_type = err_type;
        } else {
            // during initialization vulkan should not throw any errors but warnings
            // instead since most code in the swapchain is shared with runtime code,
            // add this extra code path
            self.set_warning(EGFXWarningType::InitFailed, err_str);
        }
    }

    pub fn set_error(&mut self, err_type: EGFXErrorType, err_str: &str) {
        self.set_error_extra(err_type, err_str, None);
    }

    pub fn set_warning_pre_msg(&mut self, warning_str_pre: &str) {
        if self.warning.warnings.contains(&warning_str_pre.to_string()) {
            self.warning
                .warnings
                .push_front(warning_str_pre.to_string());
        }
    }

    pub fn set_warning(&mut self, warning_type: EGFXWarningType, warning_str: &str) {
        // TODO dbg_msg("vulkan", "vulkan warning: %s", pWarning);
        if self.warning.warnings.contains(&warning_str.to_string()) {
            self.warning.warnings.push_back(warning_str.to_string());
        }
        self.warning.warning_type = warning_type;
    }
}

#[derive(Default)]
pub struct CheckResult {
    _error_helper: String,
}

impl CheckResult {
    // TODO: using options for errors is weird
    pub fn check_vulkan_critical_error(
        &mut self,
        call_result: vk::Result,
        error: &Arc<Mutex<Error>>,
        recreate_swap_chain: &mut bool,
    ) -> Option<&'static str> {
        let mut critical_error = None;
        match call_result {
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
                error.lock().unwrap().set_warning(
                    EGFXWarningType::MissingExtension,
                    "One Vulkan layer was not present. (try to disable them)",
                );
            }
            vk::Result::ERROR_EXTENSION_NOT_PRESENT => {
                error.lock().unwrap().set_warning(
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
