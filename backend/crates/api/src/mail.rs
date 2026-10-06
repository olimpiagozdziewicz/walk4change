//! SMTP email sending (via lettre, STARTTLS): magic-link login and
//! e-mail verification (spec 2026-07-13).

use lettre::message::header::ContentType;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

use crate::{config::MailConfig, error::AppError};

/// SMTP connect/command timeout (audit debt: lettre had no timeout set).
const SMTP_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);

/// Send an HTML email through the configured SMTP relay.
async fn send_html(
    cfg: &MailConfig,
    to_email: &str,
    subject: &str,
    body: String,
) -> Result<(), AppError> {
    let email = Message::builder()
        .from(cfg.from.parse().map_err(AppError::internal)?)
        .to(to_email.parse().map_err(AppError::internal)?)
        .subject(subject)
        .header(ContentType::TEXT_HTML)
        .body(body)
        .map_err(AppError::internal)?;

    let creds = Credentials::new(cfg.user.clone(), cfg.pass.clone());
    let mailer: AsyncSmtpTransport<Tokio1Executor> =
        AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&cfg.host)
            .map_err(AppError::internal)?
            .port(cfg.port)
            .credentials(creds)
            .timeout(Some(SMTP_TIMEOUT))
            .build();

    mailer.send(email).await.map_err(AppError::internal)?;
    Ok(())
}

/// Shared button-mail layout (brand colors as in the app).
fn button_mail(heading: &str, intro: &str, link: &str, cta: &str, footer: &str) -> String {
    format!(
        "<div style=\"font-family:system-ui,sans-serif;max-width:480px;margin:auto\">\
           <h2 style=\"color:#0c5a71\">{heading}</h2>\
           <p>{intro}</p>\
           <p><a href=\"{link}\" style=\"display:inline-block;padding:12px 22px;background:#0f8b8d;\
           color:#fff;border-radius:12px;text-decoration:none;font-weight:700\">{cta}</a></p>\
           <p style=\"color:#94a3b8;font-size:12px\">{footer}</p>\
         </div>"
    )
}

/// Send a magic-link login email to `to_email` with the given login `link`.
pub async fn send_magic_link(cfg: &MailConfig, to_email: &str, link: &str) -> Result<(), AppError> {
    let body = button_mail(
        "Witaj w SeaSteps 🌊",
        "Kliknij poniższy przycisk, aby się zalogować. Link wygasa za 15 minut.",
        link,
        "Zaloguj się",
        "Jeśli to nie Ty prosiłeś o logowanie, zignoruj tę wiadomość.",
    );
    send_html(cfg, to_email, "Twój magiczny link do SeaSteps", body).await
}

/// Send an e-mail verification message with the given confirmation `link`.
pub async fn send_verification_email(
    cfg: &MailConfig,
    to_email: &str,
    link: &str,
) -> Result<(), AppError> {
    let body = button_mail(
        "Potwierdź swój e-mail 🌊",
        "Kliknij poniższy przycisk, aby potwierdzić adres e-mail w SeaSteps. \
         Potwierdzony e-mail odblokowuje otwarte spacery. Link wygasa za 24 godziny.",
        link,
        "Potwierdzam e-mail",
        "Jeśli to nie Ty zakładałeś konto w SeaSteps, zignoruj tę wiadomość.",
    );
    send_html(cfg, to_email, "Potwierdź e-mail w SeaSteps", body).await
}

/// Notify the moderation mailbox about a new content report (UGC policy,
/// 2026-10-01). Deliberately carries NO reported content, note or reporter
/// identity — only the report id and target type; details live in the DB
/// (`content_reports`).
pub async fn send_report_notification(
    cfg: &MailConfig,
    to_email: &str,
    report_id: uuid::Uuid,
    target_type: &str,
) -> Result<(), AppError> {
    let body = format!(
        "<div style=\"font-family:system-ui,sans-serif;max-width:480px;margin:auto\">           <h2 style=\"color:#0c5a71\">Nowe zgłoszenie treści</h2>           <p>Typ celu: <b>{target_type}</b><br>ID zgłoszenia: <code>{report_id}</code></p>           <p>Szczegóły w tabeli <code>content_reports</code> (status <code>open</code>).            Regulamin pkt 7: rozpatrzenie bez zbędnej zwłoki.</p>         </div>"
    );
    send_html(cfg, to_email, "SeaSteps: nowe zgłoszenie treści", body).await
}
