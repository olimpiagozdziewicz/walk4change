//! SMTP email sending for magic-link login (via lettre, STARTTLS).

use lettre::message::header::ContentType;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

use crate::{config::MailConfig, error::AppError};

/// Send a magic-link login email to `to_email` with the given login `link`.
pub async fn send_magic_link(cfg: &MailConfig, to_email: &str, link: &str) -> Result<(), AppError> {
    let body = format!(
        "<div style=\"font-family:system-ui,sans-serif;max-width:480px;margin:auto\">\
           <h2 style=\"color:#0c5a71\">Witaj w SeaSteps 🌊</h2>\
           <p>Kliknij poniższy przycisk, aby się zalogować. Link wygasa za 15 minut.</p>\
           <p><a href=\"{link}\" style=\"display:inline-block;padding:12px 22px;background:#0f8b8d;\
           color:#fff;border-radius:12px;text-decoration:none;font-weight:700\">Zaloguj się</a></p>\
           <p style=\"color:#94a3b8;font-size:12px\">Jeśli to nie Ty prosiłeś o logowanie, zignoruj tę wiadomość.</p>\
         </div>"
    );

    let email = Message::builder()
        .from(cfg.from.parse().map_err(AppError::internal)?)
        .to(to_email.parse().map_err(AppError::internal)?)
        .subject("Twój magiczny link do SeaSteps")
        .header(ContentType::TEXT_HTML)
        .body(body)
        .map_err(AppError::internal)?;

    let creds = Credentials::new(cfg.user.clone(), cfg.pass.clone());
    let mailer: AsyncSmtpTransport<Tokio1Executor> =
        AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&cfg.host)
            .map_err(AppError::internal)?
            .port(cfg.port)
            .credentials(creds)
            .build();

    mailer.send(email).await.map_err(AppError::internal)?;
    Ok(())
}
