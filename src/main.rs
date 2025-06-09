use crossterm::{
    event::{self, Event as CrosstermEvent, KeyCode},
    execute,
    terminal::{
        disable_raw_mode, enable_raw_mode, EnterAlternateScreen,
        LeaveAlternateScreen,
    },
};
use ratatui::{
    backend::CrosstermBackend,
    Terminal,
};
use std::{
    io::{self, stdout},
    sync::{Arc, Mutex},
    time::Duration,
};
use std::path::PathBuf;
use tokio::time::sleep;
use tokio::sync::mpsc;

mod app;
mod ui;
mod config;
mod data;
mod http;

use app::{App, AppMode};
use config::Config;
use data::DataManager;

// 各タスク間でやり取りするイベントの種類を定義
#[derive(Debug)]
enum AppEvent {
    Crossterm(CrosstermEvent),
    Tick,
    // API呼び出しをトリガーするイベント。
    // is_first_call: 初回呼び出しを示すフラグ（trueの場合はステータスのみ、falseの場合はJSON保存）
    ApiCallTriggered {
        endpoint: String,
        is_first_call: bool,
        json_dir: Option<PathBuf>,
    },
    ApiCallCompleted(String), // API呼び出し完了メッセージ
}

#[tokio::main]
async fn main() -> io::Result<()> {
    // ターミナルセットアップ
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // --- Configの読み込み ---
    let config_path = "config.json";
    let app: Arc<Mutex<App>>; // AppのArc<Mutex>を宣言

    let config_load_result = Config::load_from_file(config_path);

    // Configの読み込み結果に応じてAppを初期化
    match config_load_result {
        Ok(cfg) => {
            eprintln!("Config loaded successfully: {:?}", cfg);
            let initial_mode = if cfg.on_time { AppMode::OnTimeMode } else { AppMode::ClockMode };
            app = Arc::new(Mutex::new(App::new(
                initial_mode,
                cfg.time.h,
                cfg.time.m,
                cfg.time.s,
                cfg.api.clone(),
            )));
        },
        Err(e) => {
            eprintln!("Failed to load config: {}. Application will start in an error state.", e);
            app = Arc::new(Mutex::new(App::new(
                AppMode::ClockMode, // デフォルトモード (エラー表示のみで機能しない)
                0, 0, 0, // 時間も0に
                "".to_string(), // APIエンドポイントも空に
            )));
            app.lock().unwrap().set_error(format!("設定ファイルの読み込みに失敗しました: {}. 機能を停止します。", e));
        }
    };


    // --- 初回起動時のディレクトリセットアップ ---
    let mut app_guard = app.lock().unwrap(); // ロックを一回取得
    let mut should_trigger_initial_api_call = false; // 初回API呼び出しをトリガーするかどうかのフラグ

    if !app_guard.api_endpoint.is_empty() { // Configが正常に読み込まれた場合のみ実行
        drop(app_guard); // ロックを解放
        let today_dir_result = DataManager::setup_directories().await;
        app_guard = app.lock().unwrap(); // 再度ロック
        match today_dir_result {
            Ok(path) => {
                app_guard.today_json_dir = Some(path.clone());
                app_guard.set_status_message(format!("データディレクトリ '{}' をセットアップしました。", path.display()));
                should_trigger_initial_api_call = true; // ディレクトリセットアップ成功時に初回API呼び出しを許可
            },
            Err(e) => {
                app_guard.set_error(format!("データディレクトリのセットアップに失敗しました: {}", e));
            }
        }
    } else {
        // Configエラーの場合はディレクトリセットアップも試みない
        app_guard.set_error("設定ファイルに問題があるため、データディレクトリのセットアップはスキップされました。".to_string());
    }
    drop(app_guard); // ロックを解放


    // 定刻モードの場合、次回のトリガー時刻を設定
    { // ロックのスコープ
        let mut app_guard = app.lock().unwrap();
        if !app_guard.api_endpoint.is_empty() && app_guard.today_json_dir.is_some() && app_guard.mode == AppMode::OnTimeMode {
            app_guard.set_next_trigger_time();
        }
    } // ロックを解放


    // チャネルの作成
    let (event_tx, mut event_rx) = mpsc::channel(100);


    // --- 初回API呼び出しのトリガー ---
    // Config読み込みとディレクトリセットアップが成功した場合のみ
    if should_trigger_initial_api_call {
        let current_endpoint = app.lock().unwrap().api_endpoint.clone();
        let json_dir = app.lock().unwrap().today_json_dir.clone();
        let is_first = app.lock().unwrap().is_first_api_call; // 初回フラグを取得

        // AppEvent::ApiCallTriggered イベントを送信し、is_first_call を含める
        if event_tx.send(AppEvent::ApiCallTriggered {
            endpoint: current_endpoint,
            is_first_call: is_first,
            json_dir,
        }).await.is_err() {
            eprintln!("Failed to send initial API call trigger.");
            if let Ok(mut app_guard) = app.lock() {
                app_guard.set_error("初期API呼び出しトリガーの送信に失敗しました。".to_string());
            }
        } else {
            if let Ok(mut app_guard) = app.lock() {
                app_guard.set_status_message("アプリケーション起動: 初回API呼び出しをトリガーしました。".to_string());
            }
        }
    }


    // --- 各非同期タスクの起動 ---

    // 1. Crosstermイベントリスナータスク (常に起動)
    let event_tx_clone_crossterm = event_tx.clone();
    tokio::spawn(async move {
        loop {
            if event::poll(Duration::from_millis(50)).unwrap() {
                if let Ok(crossterm_event) = event::read() {
                    if event_tx_clone_crossterm.send(AppEvent::Crossterm(crossterm_event)).await.is_err() {
                        break;
                    }
                }
            }
        }
    });

    // 2. タイマー更新タスク (APIエンドポイントとディレクトリが設定されている場合のみ、実質的に機能する)
    let app_clone_tick = Arc::clone(&app);
    let event_tx_clone_tick = event_tx.clone();
    tokio::spawn(async move {
        loop {
            sleep(Duration::from_secs(1)).await;

            let api_trigger_params: Option<(String, bool, Option<PathBuf>)> = {
                let mut app_guard = app_clone_tick.lock().unwrap();
                app_guard.update_time(); // 時間は常に更新

                let mut params: Option<(String, bool, Option<PathBuf>)> = None;

                // APIエンドポイントが空でない、かつJSON保存ディレクトリが設定されている場合のみトリガー判定を行う
                if !app_guard.api_endpoint.is_empty() && app_guard.today_json_dir.is_some() {
                    // is_first_api_callがtrueの場合はタイマーによるAPI呼び出しは行わない
                    // 初回API呼び出しは起動時にAppEvent::ApiCallTriggeredで処理されるため
                    if app_guard.is_first_api_call {
                        // 何もしない
                    } else if app_guard.mode == AppMode::OnTimeMode {
                        if let Some(next_trigger) = app_guard.next_trigger_time {
                            let now = chrono::Local::now().naive_local();
                            if now >= next_trigger {
                                app_guard.set_status_message(format!("定刻モード: {}にAPI実行をトリガーします。", next_trigger.format("%H:%M:%S")));
                                params = Some((
                                    app_guard.api_endpoint.clone(),
                                    false, // タイマーからの呼び出しは常に初回ではない
                                    app_guard.today_json_dir.clone(),
                                ));
                                app_guard.set_next_trigger_time();
                            }
                        }
                    } else if app_guard.mode == AppMode::ClockMode {
                        app_guard.decrement_timer();
                        if app_guard.remaining_duration.num_seconds() <= 0 {
                            app_guard.set_status_message("クロックモード: タイマーが0になりました。API実行をトリガーします。".to_string());
                            params = Some((
                                app_guard.api_endpoint.clone(),
                                false, // タイマーからの呼び出しは常に初回ではない
                                app_guard.today_json_dir.clone(),
                            ));
                            app_guard.reset_timer();
                        }
                    }
                }
                params
            };

            // ここで直接 http::fetch_api_data を呼び出す代わりに、イベントを送信する
            if let Some((endpoint, is_first_call, json_dir)) = api_trigger_params {
                if event_tx_clone_tick.send(AppEvent::ApiCallTriggered {
                    endpoint,
                    is_first_call,
                    json_dir,
                }).await.is_err() {
                    eprintln!("Failed to send API call trigger from timer task.");
                }
            }

            // このTickイベントは毎秒UIを更新する目的で継続
            if event_tx_clone_tick.send(AppEvent::Tick).await.is_err() {
                break;
            }
        }
    });

    // 3. APIアクセスワーカータスク（このタスクは不要だが、以前の構造に合わせて残す）
    let _ = tokio::spawn(async move {});


    // 4. メインアプリケーションループ (UI描画とイベント処理)
    loop {
        // UI描画
        terminal.draw(|frame| {
            let mut app_guard = app.lock().unwrap();
            ui::ui(frame, &mut *app_guard);
        })?;

        // イベント処理
        if let Some(event) = event_rx.recv().await {
            let mut app_guard = app.lock().unwrap();
            let current_app = &mut *app_guard;

            match event {
                AppEvent::Crossterm(crossterm_event) => {
                    let log_area_height = terminal.size()?.height;
                    current_app.handle_event(&crossterm_event, log_area_height);
                    if let CrosstermEvent::Key(key) = crossterm_event {
                        match key.code {
                            KeyCode::Char('q') => {
                                current_app.running = false;
                            }
                            _ => {} // モード切り替えキーは削除済み
                        }
                    }
                }
                AppEvent::Tick => {
                    // 何もしない
                }
                // AppEvent::ApiCallTriggered イベントのハンドラーを一本化
                AppEvent::ApiCallTriggered { endpoint, is_first_call, json_dir } => {
                    // API呼び出しがトリガーされたら、実際にAPIを呼び出すタスクを起動
                    let app_clone_for_http = Arc::clone(&app);
                    let api_tx_clone_for_http = event_tx.clone();

                    tokio::spawn(async move {
                        let result_msg = http::fetch_api_data(
                            is_first_call, // イベントから受け取ったフラグをそのまま渡す
                            endpoint,
                            json_dir,
                            app_clone_for_http,
                        ).await;
                        if api_tx_clone_for_http.send(AppEvent::ApiCallCompleted(result_msg)).await.is_err() {
                            eprintln!("Failed to send API call result from http module.");
                        }
                    });
                }
                AppEvent::ApiCallCompleted(msg) => {
                    current_app.add_log(format!("{}", msg));
                }
            }

            if !current_app.running {
                break;
            }
        }
    }

    // ターミナルをクリーンアップ
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}