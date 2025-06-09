use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::Text,
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::app::{App, AppMode};
use chrono::Local;

pub fn ui(frame: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3), // 現在時刻
            Constraint::Length(4), // API実行情報
            Constraint::Length(3), // ステータス
            Constraint::Min(0),    // ログ
        ])
        .split(frame.area());

    // --- 現在時刻の表示 ---
    let time_block = Block::default()
        .title("日本の現在時刻")
        .borders(Borders::ALL);

    let time_paragraph = Paragraph::new(Text::raw(&app.current_time))
        .block(time_block)
        .alignment(ratatui::layout::Alignment::Center);

    frame.render_widget(time_paragraph, chunks[0]);

    // --- API実行情報の表示 ---
    let mode_detail_block = Block::default()
        .title("API実行情報")
        .borders(Borders::ALL);

    let mode_detail_text = match app.mode {
        AppMode::OnTimeMode => {
            let initial_time_str = format!("{:02}時{:02}分{:02}秒", app.initial_h, app.initial_m, app.initial_s);
            let mut next_execution_str = "計算中...".to_string();

            if let Some(next_trigger) = app.next_trigger_time {
                let now = Local::now().naive_local();
                if next_trigger > now {
                    let duration_until_next = next_trigger.signed_duration_since(now);
                    let total_seconds = duration_until_next.num_seconds().max(0);
                    let h = total_seconds / 3600;
                    let m = (total_seconds % 3600) / 60;
                    let s = total_seconds % 60;
                    next_execution_str = format!("あと{:02}時間{:02}分{:02}秒", h, m, s);
                } else {
                    next_execution_str = "実行時刻を過ぎました".to_string();
                }
            }
            format!("設定時刻: {}\n次の実行まで: {}", initial_time_str, next_execution_str)
        }
        AppMode::ClockMode => {
            let total_seconds = app.total_duration.num_seconds();
            let total_h = total_seconds / 3600;
            let total_m = (total_seconds % 3600) / 60;
            let total_s = total_seconds % 60;

            let remaining_seconds = app.remaining_duration.num_seconds();
            let effective_remaining_seconds = remaining_seconds.max(0);
            let remaining_h = effective_remaining_seconds / 3600;
            let remaining_m = (effective_remaining_seconds % 3600) / 60;
            let remaining_s = effective_remaining_seconds % 60;

            format!(
                "設定周期: {:02}時間{:02}分{:02}秒\n次の実行まで: {:02}時間{:02}分{:02}秒",
                total_h, total_m, total_s,
                remaining_h, remaining_m, remaining_s
            )
        }
    };

    let mode_detail_paragraph = Paragraph::new(Text::raw(mode_detail_text))
        .block(mode_detail_block)
        .alignment(ratatui::layout::Alignment::Center);

    frame.render_widget(mode_detail_paragraph, chunks[1]);

    // --- ステータス表示 ---
    let status_chunk_index = 2;
    let status_block = Block::default()
        .title("ステータス")
        .borders(Borders::ALL);

    let status_paragraph = if let Some(msg) = &app.error_message {
        Paragraph::new(Text::raw(msg))
            .block(status_block)
            .alignment(ratatui::layout::Alignment::Center)
            .style(Style::default().fg(Color::Red))
    } else if let Some(msg) = &app.status_message {
        Paragraph::new(Text::raw(msg))
            .block(status_block)
            .alignment(ratatui::layout::Alignment::Center)
            .style(Style::default().fg(Color::Yellow))
    } else {
        Paragraph::new(Text::raw("待機中..."))
            .block(status_block)
            .alignment(ratatui::layout::Alignment::Center)
            .style(Style::default().fg(Color::DarkGray))
    };

    frame.render_widget(status_paragraph, chunks[status_chunk_index]);

    // --- ログ表示 ---
    let log_chunk_index = 3;
    if chunks.len() > log_chunk_index {
        let log_area = chunks[log_chunk_index];
        let log_content_area = log_area; // スクロールバーがないため、ログ本体がログエリア全体を使用

        // ログ表示領域の実際の高さを取得（ボーダー分を引く）
        let display_height = log_content_area.height.saturating_sub(2) as usize;
        // ログ全体の行数
        let total_log_lines = app.logs.len();

        // スクロール可能な最大位置
        let max_scroll_position = total_log_lines.saturating_sub(display_height).max(0);

        // app.log_scroll の値を適切に調整し、常に有効な範囲に保つ
        if app.is_log_auto_scroll {
            app.log_scroll = max_scroll_position;
        } else {
            app.log_scroll = app.log_scroll.min(max_scroll_position).max(0);
        }

        // ページ計算
        let current_page = if display_height == 0 { // 表示可能な行がない場合
            0
        } else {
            // 現在のスクロール位置 / 1ページあたりの行数 + 1
            // ログが0行の場合も1ページ目として扱う
            (app.log_scroll / display_height) + 1
        };

        let total_pages = if display_height == 0 { // 表示可能な行がない場合
            0
        } else {
            // (ログ総行数 + 1ページあたりの行数 - 1) / 1ページあたりの行数
            // 例えば、10行表示でログが11行なら2ページ
            (total_log_lines + display_height - 1) / display_height
        };

        // ログブロックのタイトルにページ情報を追加
        let log_title = format!("ログ ({}/{})", current_page, total_pages);
        let log_block = Block::default()
            .title(log_title)
            .borders(Borders::ALL);

        // 表示するログの範囲を決定
        let start_index = app.log_scroll;
        let end_index = (start_index + display_height).min(total_log_lines);

        let visible_logs: Vec<String> = app.logs.iter()
            .skip(start_index)
            .take(end_index.saturating_sub(start_index))
            .cloned()
            .collect();

        let log_text = Text::from(visible_logs.join("\n"));

        let log_paragraph = Paragraph::new(log_text)
            .block(log_block)
            .alignment(ratatui::layout::Alignment::Left)
            .scroll((0, 0));

        frame.render_widget(log_paragraph, log_content_area);
    }
}