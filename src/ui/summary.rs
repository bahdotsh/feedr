use crate::app::App;
use crate::ui::ColorScheme;
use ratatui::{
    backend::Backend,
    layout::{Alignment, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Padding, Paragraph, Wrap},
    Frame,
};

pub(super) fn render_summary<B: Backend>(
    f: &mut Frame<B>,
    app: &App,
    area: Rect,
    colors: &ColorScheme,
) {
    let (total_new, feeds_with_counts) = app.get_summary_stats();
    let feed_count = feeds_with_counts.len();

    let summary_icon = if colors.border_normal == BorderType::Double {
        "◈"
    } else {
        "✦"
    };

    let title = format!(
        " {} What's New - {} new item{} across {} feed{} ",
        summary_icon,
        total_new,
        if total_new == 1 { "" } else { "s" },
        feed_count,
        if feed_count == 1 { "" } else { "s" },
    );

    let mut lines: Vec<Line> = Vec::new();

    lines.push(Line::from(""));
    lines.push(Line::from(vec![Span::styled(
        format!("  {} What's New Since Last Visit", summary_icon),
        Style::default()
            .fg(colors.primary)
            .add_modifier(Modifier::BOLD),
    )]));
    lines.push(Line::from(""));

    // Per-feed breakdown
    let arrow = colors.get_arrow_right();
    for (feed_name, count) in &feeds_with_counts {
        lines.push(Line::from(vec![
            Span::styled(
                format!("  {} ", arrow),
                Style::default().fg(colors.highlight),
            ),
            Span::styled(
                feed_name.clone(),
                Style::default()
                    .fg(colors.text)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  {} new item{}", count, if *count == 1 { "" } else { "s" }),
                Style::default().fg(colors.text_secondary),
            ),
        ]));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(""));
    lines.push(Line::from(vec![Span::styled(
        "  Press any key to continue to Dashboard",
        Style::default().fg(colors.muted),
    )]));

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .title(title)
                .title_alignment(Alignment::Center)
                .borders(Borders::ALL)
                .border_type(colors.border_normal)
                .border_style(Style::default().fg(colors.border))
                .style(Style::default().bg(colors.surface))
                .padding(Padding::new(2, 2, 1, 1)),
        )
        .wrap(Wrap { trim: true });

    f.render_widget(paragraph, area);
}
