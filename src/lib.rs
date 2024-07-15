use dotenv::dotenv;
use flowsnet_platform_sdk::logger;
use openai_flows::{
    chat::{ChatModel, ChatOptions},
    OpenAIFlows,
};
use slack_flows::{listen_to_channel, send_message_to_channel, SlackMessage};
use std::env;
use reqwest::Client;
use serde_json::json;

#[no_mangle]
#[tokio::main(flavor = "current_thread")]
pub async fn run() {
    dotenv().ok();
    logger::init();
    let workspace: String = match env::var("slack_workspace") {
        Err(_) => "secondstate".to_string(),
        Ok(name) => name,
    };

    let channel: String = match env::var("slack_channel") {
        Err(_) => "collaborative-chat".to_string(),
        Ok(name) => name,
    };

    log::debug!("Workspace is {} and channel is {}", workspace, channel);

    listen_to_channel(&workspace, &channel, |sm| handler(sm, &workspace, &channel)).await;
}

async fn handler(sm: SlackMessage, workspace: &str, channel: &str) {
    let chat_id = workspace.to_string() + channel;
    let co = ChatOptions {
        model: ChatModel::GPT35Turbo,
        restart: false,
        system_prompt: None,
    };

    let api_service: String = match env::var("api_service") {
        Err(_) => "openai".to_string(),
        Ok(service) => service,
    };

    log::debug!("get API settings");
    let response = match api_service.as_str() {
        "openai" => {
            let of = OpenAIFlows::new();
            of.chat_completion(&chat_id, &sm.text, &co).await
        },
        "custom" => {
            let api_url = env::var("custom_api_url").unwrap_or_else(|_| "https://aigptx.top/v1".to_string());
            let api_key = env::var("custom_api_key").expect("Custom API Key not set");
            let client = Client::new();
            let res = client.post(&api_url)
                .header("Authorization", format!("Bearer {}", api_key))
                .json(&json!({
                    "model": co.model,
                    "messages": [{"role": "user", "content": sm.text}]
                }))
                .send()
                .await;

            match res {
                Ok(resp) => {
                    let json: serde_json::Value = resp.json().await.unwrap();
                    Ok(json["choices"][0]["message"]["content"].as_str().unwrap().to_string())
                },
                Err(_) => Err("Error with custom API".to_string())
            }
        },
        _ => Err("Unknown API service".to_string()),
    };

    if let Ok(c) = response {
        log::debug!("got response from API");
        send_message_to_channel(&workspace, &channel, c).await;
        log::debug!("sent to slack");
    }
    log::debug!("done");
}
