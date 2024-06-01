use std::{collections::HashSet, sync::Arc, time::Duration};

use accounts_base::account_server::otp::{generate_otp, Otp};

pub const OTP_TIMEOUT: Duration = Duration::from_secs(20);

/// Manager for one time passwords.
/// Automatically removes them after a timeout hits
#[derive(Debug, Default)]
pub struct Otps {
    active_otps: Arc<parking_lot::Mutex<HashSet<Otp>>>,
}

impl Otps {
    /// Generates a new one time password.
    /// Allowing operations with it for a specific time
    /// ([`OTP_TIMEOUT`]).
    pub fn gen_otp(&self) -> Otp {
        let otp = generate_otp();
        let otps = self.active_otps.clone();
        // remove otp after some time
        tokio::spawn(async move {
            tokio::time::sleep(OTP_TIMEOUT).await;
            otps.lock().remove(&otp);
        });
        // add otp to active list
        self.active_otps.lock().insert(otp);
        otp
    }

    /// Checks if the otp exists, removes it from active otps
    /// and returns if that otp existed.
    /// If this returns `true`, that basically means the client
    /// send the correct password.
    pub fn try_consume_otp(&self, otp: Otp) -> bool {
        self.active_otps.lock().remove(&otp)
    }
}
