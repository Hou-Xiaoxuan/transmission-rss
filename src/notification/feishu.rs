use serde::Serialize;
use std::error::Error as StdError;

use reqwest::StatusCode;

use super::notification::Error;

pub struct FeiShu {
    webhook: String,
}

#[derive(Serialize)]
pub struct FeiShuMessage {
    msg_type: String,
    content: FeiShuContent,
}

#[derive(Serialize)]
#[serde(untagged)]
enum FeiShuContent {
    Text { text: String },
}

impl FeiShu {
    pub fn new(webhook: String) -> Self {
        Self { webhook }
    }

    pub async fn send(&self, message: String) -> Result<(), Box<dyn StdError>> {
        let client = reqwest::Client::new();
        let res = client
            .post(&self.webhook)
            .header("Content-Type", "application/json")
            .json(&FeiShuMessage {
                msg_type: "text".to_string(),
                content: FeiShuContent::Text {
                    text: message.to_owned(),
                },
            })
            .send()
            .await?;

        if res.status() != StatusCode::OK {
            if let Ok(val) = res.text().await {
                dbg!(&val);

                return Err(Box::new(Error::new(val)));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[tokio::test]
    async fn test_message() {
        println!("over");
        let msg = json!(&FeiShuMessage {
            msg_type: "text".to_string(),
            content: FeiShuContent::Text {
                text: "test".to_owned(),
            },
        });
        assert!(serde_json::to_string(&msg).is_ok());
        let msg_str = serde_json::to_string(&msg).unwrap();
        assert!(msg_str == "{\"content\":{\"text\":\"test\"},\"msg_type\":\"text\"}");
    }
}
