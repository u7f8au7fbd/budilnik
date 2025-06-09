// src/http.rs

use reqwest::Client;
use std::path::PathBuf;
use crate::data::DataManager; // dataモジュールをインポート
use crate::app::App; // Appの状態を更新するためにインポート
use std::sync::{Arc, Mutex}; // Arc<Mutex<App>> を受け取るために必要

/// API呼び出しのロジックをカプセル化する
///
/// is_first_call: API呼び出しが初回かどうか (初回はステータスのみ、次回以降はJSON保存)
/// endpoint: APIのエンドポイントURL
/// today_json_dir: JSON保存先ディレクトリのパス (Option<PathBuf> で None の場合も考慮)
/// app_state: Appの状態を更新するための Arc<Mutex<App>>
pub async fn fetch_api_data(
    is_first_call: bool,
    endpoint: String,
    today_json_dir: Option<PathBuf>,
    app_state: Arc<Mutex<App>>, // Appの状態を更新するために追加
) -> String {
    let client = Client::new();
    let log_message: String; // ここを修正: 初期化を省略し、型のみを宣言

    if is_first_call {
        // 初回API呼び出し: HTTPステータスのみ表示
        match client.get(&endpoint).send().await {
            Ok(response) => {
                log_message = format!("初回API呼び出し完了 (ステータス: {})", response.status());
                // Appのis_first_api_callフラグをここでfalseに設定
                if let Ok(mut app_guard) = app_state.lock() {
                    app_guard.is_first_api_call = false;
                    app_guard.set_status_message(format!("初回API呼び出し成功: ステータス {}", response.status()));
                }
            }
            Err(e) => {
                log_message = format!("初回API呼び出し失敗: {}", e);
                if let Ok(mut app_guard) = app_state.lock() {
                    app_guard.set_error(format!("初回API呼び出し失敗: {}", e));
                }
            }
        }
    } else {
        // 2回目以降のAPI呼び出し: JSONを保存
        match client.get(&endpoint).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    match response.text().await {
                        Ok(json_text) => {
                            if let Some(dir) = today_json_dir {
                                match DataManager::save_api_response(&dir, &json_text).await {
                                    Ok(_) => {
                                        // JSONファイル名形式の変更に合わせてここも修正
                                        log_message = format!("API呼び出し成功: JSONを保存しました ({})", chrono::Local::now().format("%H-%M-%S").to_string());
                                    }
                                    Err(e) => {
                                        log_message = format!("API呼び出し成功、JSON保存失敗: {}", e);
                                    }
                                }
                            } else {
                                log_message = "API呼び出し成功、JSON保存ディレクトリが見つかりません。".to_string();
                            }
                        }
                        Err(e) => {
                            log_message = format!("API呼び出し成功、レスポンステキスト読み込み失敗: {}", e);
                        }
                    }
                } else {
                    log_message = format!("API呼び出しエラー: ステータス {}", response.status());
                }
            }
            Err(e) => {
                log_message = format!("API呼び出し失敗: {}", e);
            }
        }
    }
    log_message
}