// src/app.rs

use chrono::{Duration as ChronoDuration, Local, NaiveDateTime, NaiveTime};
use crossterm::event::{Event as CrosstermEvent, KeyCode};
use std::collections::VecDeque;
use std::path::PathBuf;

// アプリケーションモードの列挙型
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum AppMode {
    OnTimeMode, // 定刻モード
    ClockMode,  // クロックモード
}

// アプリケーションの状態を管理する構造体
pub struct App {
    pub current_time: String,
    pub running: bool,
    pub mode: AppMode,
    pub initial_h: u32,
    pub initial_m: u32,
    pub initial_s: u32,
    pub error_message: Option<String>,
    pub status_message: Option<String>,
    pub api_endpoint: String, // 追加: APIエンドポイント

    // 定刻モード用
    pub next_trigger_time: Option<NaiveDateTime>,

    // クロックモード用
    pub total_duration: ChronoDuration, // 設定されたタイマーの総時間
    pub remaining_duration: ChronoDuration, // 残り時間

    // ログ機能
    pub logs: VecDeque<String>, // ログ履歴を保持 (最大256個)
    pub log_scroll: usize,      // ログのスクロール位置 (表示されるログの先頭行のインデックス)
    pub max_logs: usize,        // ログの最大保持数
    pub is_log_auto_scroll: bool, // ログが自動スクロールモードかどうか

    // 新規追加
    pub is_first_api_call: bool, // API呼び出しが初回かどうかを判断するフラグ
    pub today_json_dir: Option<PathBuf>, // 今日のJSON保存ディレクトリのパス
}

impl App {
    // APIエンドポイントを引数に追加
    pub fn new(mode: AppMode, h: u32, m: u32, s: u32, api_endpoint: String) -> App {
        let total_duration = ChronoDuration::hours(h as i64)
            + ChronoDuration::minutes(m as i64)
            + ChronoDuration::seconds(s as i64);
        App {
            current_time: String::new(),
            running: true,
            mode,
            initial_h: h,
            initial_m: m,
            initial_s: s,
            error_message: None,
            status_message: None,
            api_endpoint, // ここで設定
            next_trigger_time: None,
            total_duration,
            remaining_duration: total_duration,
            logs: VecDeque::with_capacity(256), // 容量を事前に確保
            log_scroll: 0, // 初期スクロール位置は最上部
            max_logs: 256,
            is_log_auto_scroll: true, // 初期状態では自動スクロールを有効にする
            is_first_api_call: true, // 初期値はtrue
            today_json_dir: None,    // 初期値はNone
        }
    }
    
    pub fn update_time(&mut self) {
        // ここが修正箇所： %M はゼロパディングされた分、%S はゼロパディングされた秒
        // 確認のため、日本語の「分」と「秒」の文字を明示的に追加しています。
        self.current_time = Local::now().format("%Y年%m月%d日 %H時%M分%S秒").to_string();
    }

    pub fn handle_event(&mut self, event: &CrosstermEvent, log_display_height: u16) {
        if let CrosstermEvent::Key(key) = event {
            // スクロール可能な最大位置を計算
            let max_scroll_position = self.logs.len().saturating_sub(log_display_height as usize).max(0);

            match key.code {
                KeyCode::Char('q') => {
                    self.running = false;
                }
                KeyCode::Up => {
                    self.log_scroll = self.log_scroll.saturating_sub(1);
                    self.is_log_auto_scroll = false;
                }
                KeyCode::Down => {
                    self.log_scroll = self.log_scroll.saturating_add(1);
                    if self.log_scroll >= max_scroll_position {
                        self.is_log_auto_scroll = true;
                        self.log_scroll = max_scroll_position;
                    } else {
                        self.is_log_auto_scroll = false;
                    }
                }
                KeyCode::Home => {
                    self.log_scroll = 0;
                    self.is_log_auto_scroll = false;
                }
                KeyCode::End => {
                    self.log_scroll = max_scroll_position;
                    self.is_log_auto_scroll = true;
                }
                _ => {
                    // その他のキー入力は無視（モード切り替えキーは削除）
                }
            }
            self.log_scroll = self.log_scroll.min(max_scroll_position);
        }
    }

    pub fn set_error(&mut self, message: String) {
        let timestamp = Local::now().format("%H:%M:%S").to_string();
        let log_entry = format!("{}: ERROR: {}", timestamp, message);
        self.add_log(log_entry);

        self.error_message = Some(message);
        self.status_message = None;
    }

    pub fn set_status_message(&mut self, message: String) {
        let timestamp = Local::now().format("%H:%M:%S").to_string();
        let log_entry = format!("{}: {}", timestamp, message);
        self.add_log(log_entry);

        self.status_message = Some(message);
        self.error_message = None;
    }

    pub fn add_log(&mut self, log_entry: String) {
        if self.logs.len() == self.max_logs {
            self.logs.pop_front();
        }
        self.logs.push_back(log_entry);

        if self.is_log_auto_scroll {
            self.log_scroll = self.logs.len();
        }
    }

    pub fn set_next_trigger_time(&mut self) {
        let now = Local::now();
        let target_time = NaiveTime::from_hms_opt(self.initial_h, self.initial_m, self.initial_s)
            .unwrap_or_else(|| NaiveTime::from_hms_opt(0, 0, 0).unwrap());

        let mut next_trigger = now.naive_local().date().and_time(target_time);

        if next_trigger <= now.naive_local() {
            next_trigger += ChronoDuration::days(1);
        }
        self.next_trigger_time = Some(next_trigger);
    }

    pub fn reset_timer(&mut self) {
        self.remaining_duration = self.total_duration;
    }

    pub fn decrement_timer(&mut self) {
        self.remaining_duration = self.remaining_duration - ChronoDuration::seconds(1);
        if self.remaining_duration.num_seconds() < 0 {
            self.remaining_duration = ChronoDuration::seconds(0);
        }
    }
}