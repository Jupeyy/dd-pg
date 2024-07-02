use std::{fmt::Debug, sync::Arc};

use lettre::{transport::smtp::authentication::Credentials, Message, SmtpTransport, Transport};

pub trait EmailHook: Debug + Sync + Send {
    fn on_mail(&self, email_subject: &str, email_body: &str);
}

#[derive(Debug)]
struct EmailHookDummy {}
impl EmailHook for EmailHookDummy {
    fn on_mail(&self, _email_subject: &str, _email_body: &str) {
        // empty
    }
}

/// Shared email helper
#[derive(Debug)]
pub struct EmailShared {
    smtp: SmtpTransport,
    pub email_from: String,
    mail_hook: Arc<dyn EmailHook>,
}

impl EmailShared {
    pub fn new(
        relay: &str,
        relay_port: u16,
        from_email: &str,
        username: &str,
        password: &str,
    ) -> anyhow::Result<Self> {
        let smtp = SmtpTransport::relay(relay)?
            .port(relay_port)
            .credentials(Credentials::new(username.into(), password.into()))
            .build();

        anyhow::ensure!(
            smtp.test_connection()?,
            "Could not connect to smtp server: {}",
            relay
        );
        Ok(Self {
            smtp,
            mail_hook: Arc::new(EmailHookDummy {}),
            email_from: from_email.into(),
        })
    }

    /// A hook that can see all sent emails
    /// Currently only useful for testing
    #[allow(dead_code)]
    pub fn set_hook<F: EmailHook + 'static>(&mut self, hook: F) {
        self.mail_hook = Arc::new(hook);
    }

    pub async fn send_email(&self, to: &str, subject: &str, body: String) -> anyhow::Result<()> {
        self.mail_hook.on_mail(subject, &body);
        let email = Message::builder()
            .from(self.email_from.parse().unwrap())
            .to(to.parse().unwrap())
            .subject(subject)
            .body(body)
            .unwrap();
        self.smtp.send(&email)?;

        Ok(())
    }
}

impl From<(&str, SmtpTransport)> for EmailShared {
    fn from((email_from, smtp): (&str, SmtpTransport)) -> Self {
        Self {
            smtp,
            mail_hook: Arc::new(EmailHookDummy {}),
            email_from: email_from.into(),
        }
    }
}

#[cfg(test)]
mod test {
    use lettre::SmtpTransport;

    use crate::email::EmailShared;

    #[tokio::test]
    async fn email_test() {
        let email: EmailShared = ("test@localhost", SmtpTransport::unencrypted_localhost()).into();

        assert!(email.smtp.test_connection().unwrap());

        email
            .send_email(
                "TestTo <test@localhost>",
                "It works",
                "It indeed works".to_string(),
            )
            .await
            .unwrap();
    }
}
