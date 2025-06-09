use std::{fs, path::Path};

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct TimeConfig {
    pub h: u32,
    pub m: u32,
    pub s: u32,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    pub api: String,
    pub on_time: bool, // true: 定刻モード, false: クロックモード
    pub time: TimeConfig,
}

impl Config {
    pub fn load_from_file(path_str: &str) -> Result<Self> {
        let path = Path::new(path_str);

        // ファイルの存在チェック
        if !path.exists() {
            return Err(anyhow!("エラー: '{}' が見つかりません。", path_str));
        }

        // ファイルの読み込み
        let content = fs::read_to_string(path)?;

        // JSONのパースとバリデーション
        let config: Config = serde_json::from_str(&content)
            .map_err(|e| anyhow!("設定ファイルのパースエラー: {}", e))?;

        // 定刻モードの場合のバリデーション
        if config.on_time {
            if config.time.h >= 24 {
                return Err(anyhow!("設定エラー: 定刻モードでは 'time.h' は24未満である必要があります (現在: {})", config.time.h));
            }
            // 定刻モードではMとSは常に60未満
            if config.time.m >= 60 {
                return Err(anyhow!("設定エラー: 定刻モードでは 'time.m' は60未満である必要があります (現在: {})", config.time.m));
            }
            if config.time.s >= 60 {
                return Err(anyhow!("設定エラー: 定刻モードでは 'time.s' は60未満である必要があります (現在: {})", config.time.s));
            }
        } else { // クロックモードの場合
            // クロックモードのmとsの制限解除ロジック
            let h_is_zero = config.time.h == 0;
            let m_is_zero = config.time.m == 0;
            let s_is_zero = config.time.s == 0;

            // h=0, m=0 の場合、sの60制限を解除
            if ! (h_is_zero && m_is_zero) && config.time.s >= 60 {
                return Err(anyhow!("設定エラー: クロックモードでは 'time.s' は60未満である必要があります (現在: {})", config.time.s));
            }
            // h=0, s=0 の場合、mの60制限を解除
            if ! (h_is_zero && s_is_zero) && config.time.m >= 60 {
                return Err(anyhow!("設定エラー: クロックモードでは 'time.m' は60未満である必要があります (現在: {})", config.time.m));
            }
        }

        Ok(config)
    }
}