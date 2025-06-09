use anyhow::Result;
use std::path::{Path, PathBuf};
use tokio::fs::{self, File};
use tokio::io::AsyncWriteExt; // for AsyncWriteExt trait
use chrono::Local;

/// ディレクトリ構造を管理し、APIレスポンスを保存するモジュール
pub struct DataManager;

impl DataManager {
    /// 初回起動時に必要なディレクトリ構造をセットアップする
    /// ./jsons/YYYY-MM-DD/ の形式でディレクトリを生成する
    pub async fn setup_directories() -> Result<PathBuf> {
        let base_dir = PathBuf::from("./jsons");

        // ./jsons ディレクトリが存在するか確認し、なければ作成
        if !base_dir.exists() {
            fs::create_dir_all(&base_dir).await?;
        }

        // 今日の日付のディレクトリ (例: 2025-06-09) を生成
        let today_str = Local::now().format("%Y-%m-%d").to_string();
        let today_dir = base_dir.join(&today_str);

        if !today_dir.exists() {
            fs::create_dir_all(&today_dir).await?;
        }

        Ok(today_dir)
    }

    /// APIレスポンスのJSONを指定されたディレクトリに保存する
    /// ファイル名は現在の時刻 (HHmmss.json) となる
    pub async fn save_api_response(dir: &Path, json_data: &str) -> Result<()> {
        let filename = Local::now().format("%H-%M-%S").to_string();
        let filepath = dir.join(format!("{}.json", filename));

        // ファイルにJSONデータを書き込む
        let mut file = File::create(&filepath).await?;
        file.write_all(json_data.as_bytes()).await?;

        Ok(())
    }
}