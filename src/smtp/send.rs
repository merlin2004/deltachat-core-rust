//! # SMTP message sending

use super::Smtp;
use async_smtp::*;

use crate::context::Context;
use crate::events::EventType;
use std::time::Duration;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Envelope error: {}", _0)]
    EnvelopeError(#[from] async_smtp::error::Error),

    #[error("Send error: {}", _0)]
    SendError(#[from] async_smtp::smtp::error::Error),

    #[error("SMTP has no transport")]
    NoTransport,
}

impl Smtp {
    /// Send a prepared mail to recipients.
    /// On successful send out Ok() is returned.
    pub async fn send(
        &mut self,
        context: &Context,
        recipients: Vec<EmailAddress>,
        message: Vec<u8>,
        job_id: u32,
    ) -> Result<()> {
        let message_len_bytes = message.len();

        let recipients_display = recipients
            .iter()
            .map(|x| format!("{}", x))
            .collect::<Vec<String>>()
            .join(",");

        let envelope =
            Envelope::new(self.from.clone(), recipients).map_err(Error::EnvelopeError)?;
        let mail = SendableEmail::new(
            envelope,
            format!("{}", job_id), // only used for internal logging
            message,
        );

        if let Some(ref mut transport) = self.transport {
            // The timeout is 1min + 3min per MB.
            let timeout = 60 + (180 * message_len_bytes / 1_000_000) as u64;
            transport
                .send_with_timeout(mail, Some(&Duration::from_secs(timeout)))
                .await
                .map_err(Error::SendError)?;

            context.emit_event(EventType::SmtpMessageSent(format!(
                "Message len={} was smtp-sent to {}",
                message_len_bytes, recipients_display
            )));
            self.last_success = Some(std::time::SystemTime::now());

            Ok(())
        } else {
            warn!(
                context,
                "uh? SMTP has no transport, failed to send to {}", recipients_display
            );
            Err(Error::NoTransport)
        }
    }
}
