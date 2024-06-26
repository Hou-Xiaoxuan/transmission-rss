mod feishu;
mod telegram;

use crate::config::Config;
use std::error::Error as StdError;
use std::fmt;

#[derive(Debug)]
pub struct Error {
    message: String,
}

impl Error {
    pub fn new(message: String) -> Self {
        Self { message }
    }
}

impl StdError for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.message)
    }
}

/// Send notification to all enabled
pub async fn notify_all(cfg: Config, message: String) {
    // Telegram
    if let Some(bot_cfg) = cfg.notification.telegram {
        let notifier = telegram::Telegram::new(
            bot_cfg.bot_token,
            bot_cfg.chat_id,
            "https://api.telegram.org".into(),
        );
        match notifier.send(message.clone()).await {
            Ok(_) => println!("Telegram notification sent!"),
            Err(err) => println!("Failed to send telegram message: {}", err),
        }
    }

    if let Some(feishu_cfg) = cfg.notification.feishu {
        let notifier = feishu::FeiShu::new(feishu_cfg.webhook);
        match notifier.send(message.clone()).await {
            Ok(_) => println!("Feishu notification sent!"),
            Err(err) => println!("Failed to send feishu message: {}", err),
        }
    }
}
