//! Outgoing mail (verification + password reset) over plain SMTP via lettre.
//!
//! Sending happens on a background task: a slow or absent SMTP server never delays
//! or fails the request that triggered it. Without `MAIL_HOST` the mail is logged.

use std::time::Duration;

use lettre::message::Mailbox;
use lettre::message::header::ContentType;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

use crate::config::MailConfig;

#[derive(Clone)]
pub struct Mailer {
    transport: Option<AsyncSmtpTransport<Tokio1Executor>>,
    from: Mailbox,
}

impl Mailer {
    pub fn new(config: &MailConfig) -> Self {
        let transport = config.host.as_ref().map(|host| {
            // No TLS: aimed at local sinks (mailpit, MailHog, the matrix sink).
            // Put a TLS relay in front, or switch to `relay()`, for production.
            let mut builder = AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(host)
                .port(config.port)
                .timeout(Some(Duration::from_secs(10)));
            if let (Some(user), Some(pass)) = (&config.username, &config.password) {
                builder = builder.credentials(Credentials::new(user.clone(), pass.clone()));
            }
            builder.build()
        });
        let from = config.from.parse().unwrap_or_else(|_| {
            "no-reply@cliuno.local"
                .parse()
                .expect("valid fallback address")
        });
        Self { transport, from }
    }

    /// Queues a plain-text mail; errors are logged, never returned.
    pub fn send(&self, to: &str, subject: &str, body: String) {
        let Some(transport) = self.transport.clone() else {
            tracing::info!(%to, %subject, "MAIL_HOST not set; mail not sent:\n{body}");
            return;
        };
        let message = to
            .parse::<Mailbox>()
            .map_err(|e| e.to_string())
            .and_then(|to| {
                Message::builder()
                    .from(self.from.clone())
                    .to(to)
                    .subject(subject)
                    .header(ContentType::TEXT_PLAIN)
                    .body(body)
                    .map_err(|e| e.to_string())
            });
        match message {
            Ok(message) => {
                tokio::spawn(async move {
                    if let Err(err) = transport.send(message).await {
                        tracing::warn!("failed to send mail: {err}");
                    }
                });
            }
            Err(err) => tracing::warn!(%to, "could not build mail: {err}"),
        }
    }
}
