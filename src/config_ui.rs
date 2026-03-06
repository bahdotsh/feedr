use crate::config_tui::{get_fields, ConfirmChoice, ConfigEditor, ConfigSection, FieldKind};
use ratatui::{
    backend::Backend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState, Padding, Paragraph, Tabs},
    Frame,
};

pub fn render<B: Backend>(f: &mut Frame<B>, editor: &ConfigEditor) {
    let cs = &editor.color_scheme;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // tabs
            Constraint::Min(5),   // content
            Constraint::Length(2), // help bar
        ])
        .split(f.size());

    // Section tabs
    let titles: Vec<Line> = ConfigSection::ALL
        .iter()
        .map(|s| {
            let style = if *s == editor.section {
                Style::default()
                    .fg(cs.primary)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(cs.text_secondary)
            };
            Line::from(Span::styled(s.title(), style))
        })
        .collect();

    let tabs = Tabs::new(titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(cs.border_normal)
                .border_style(Style::default().fg(cs.border))
                .title(Span::styled(
                    " feedr config ",
                    Style::default()
                        .fg(cs.primary)
                        .add_modifier(Modifier::BOLD),
                )),
        )
        .select(editor.section.index())
        .style(Style::default().fg(cs.text_secondary))
        .highlight_style(
            Style::default()
                .fg(cs.primary)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        );
    f.render_widget(tabs, chunks[0]);

    // Field list
    let fields = get_fields(editor.section, &editor.config);
    let items: Vec<ListItem> = fields
        .iter()
        .enumerate()
        .map(|(i, field)| {
            let is_selected = i == editor.selected_field;
            let is_editing = is_selected && editor.editing;

            let value_str = if is_editing {
                let cursor_pos = editor.edit_cursor;
                let before = &editor.edit_buffer[..cursor_pos];
                let cursor_char = editor.edit_buffer.chars().nth(cursor_pos).unwrap_or(' ');
                let after = if cursor_pos < editor.edit_buffer.len() {
                    &editor.edit_buffer[cursor_pos + cursor_char.len_utf8()..]
                } else {
                    ""
                };
                format!("{}█{}", before, after)
            } else {
                field.value.clone()
            };

            let kind_hint = match field.kind {
                FieldKind::Bool => " ◉",
                FieldKind::Enum => " ⟳",
                FieldKind::Text => "",
            };

            let label_style = if is_selected {
                Style::default()
                    .fg(cs.text)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(cs.text_secondary)
            };

            let value_style = if is_editing {
                Style::default()
                    .fg(cs.primary)
                    .add_modifier(Modifier::BOLD)
            } else if is_selected {
                Style::default().fg(cs.highlight)
            } else {
                Style::default().fg(cs.accent)
            };

            let desc_style = Style::default().fg(cs.muted);

            let mut spans = vec![
                Span::styled(
                    if is_selected { "▸ " } else { "  " },
                    Style::default().fg(cs.primary),
                ),
                Span::styled(format!("{:<28}", field.label), label_style),
                Span::styled(format!(" {}", value_str), value_style),
                Span::styled(kind_hint, Style::default().fg(cs.muted)),
            ];

            if !field.description.is_empty() {
                spans.push(Span::styled(
                    format!("  ({})", field.description),
                    desc_style,
                ));
            }

            ListItem::new(Line::from(spans))
        })
        .collect();

    let dirty_marker = if editor.dirty { " [modified]" } else { "" };
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(cs.border_normal)
                .border_style(Style::default().fg(cs.border))
                .title(Span::styled(
                    format!(" {} {}", editor.section.title(), dirty_marker),
                    Style::default()
                        .fg(cs.text)
                        .add_modifier(Modifier::BOLD),
                ))
                .padding(Padding::new(1, 1, 1, 0)),
        )
        .highlight_style(
            Style::default()
                .bg(cs.selected_bg)
                .add_modifier(Modifier::BOLD),
        );

    let mut state = ListState::default();
    state.select(Some(editor.selected_field));
    f.render_stateful_widget(list, chunks[1], &mut state);

    // Help bar
    let help_text = if editor.editing {
        "Enter: confirm | Esc: cancel | ←→: move cursor"
    } else if editor.section == ConfigSection::DefaultFeeds {
        "j/k: navigate | Tab/Shift-Tab: section | a: add feed | d: delete | s: save | q: quit"
    } else {
        "j/k: navigate | Tab/Shift-Tab: section | Enter/e: edit | Space: toggle | s: save | q: quit"
    };

    let help = Paragraph::new(Line::from(Span::styled(
        help_text,
        Style::default().fg(cs.text_secondary),
    )))
    .alignment(Alignment::Center);
    f.render_widget(help, chunks[2]);

    // Error/Success overlay
    if let Some(ref msg) = editor.error {
        let area = centered_rect(60, 5, f.size());
        f.render_widget(Clear, area);
        let p = Paragraph::new(Line::from(Span::styled(
            msg.as_str(),
            Style::default().fg(cs.error).add_modifier(Modifier::BOLD),
        )))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(cs.error))
                .title(" Error "),
        )
        .alignment(Alignment::Center);
        f.render_widget(p, area);
    } else if let Some(ref msg) = editor.success {
        let area = centered_rect(60, 5, f.size());
        f.render_widget(Clear, area);
        let p = Paragraph::new(Line::from(Span::styled(
            msg.as_str(),
            Style::default()
                .fg(cs.success)
                .add_modifier(Modifier::BOLD),
        )))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(cs.success))
                .title(" Success "),
        )
        .alignment(Alignment::Center);
        f.render_widget(p, area);
    }

    // Confirm quit dialog
    if editor.confirm_quit {
        let area = centered_rect(50, 7, f.size());
        f.render_widget(Clear, area);

        let inner = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2),
                Constraint::Length(2),
                Constraint::Min(0),
            ])
            .margin(1)
            .split(area);

        let msg = Paragraph::new(Line::from(Span::styled(
            "You have unsaved changes.",
            Style::default().fg(cs.text).add_modifier(Modifier::BOLD),
        )))
        .alignment(Alignment::Center);

        let btn_style = Style::default().fg(cs.text_secondary);
        let sel_style = Style::default()
            .fg(cs.primary)
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED);

        let buttons = Line::from(vec![
            Span::styled(
                " [Save & Quit] ",
                if editor.confirm_selection == ConfirmChoice::Save {
                    sel_style
                } else {
                    btn_style
                },
            ),
            Span::styled("  ", Style::default()),
            Span::styled(
                " [Discard & Quit] ",
                if editor.confirm_selection == ConfirmChoice::Discard {
                    sel_style
                } else {
                    btn_style
                },
            ),
            Span::styled("  ", Style::default()),
            Span::styled(
                " [Cancel] ",
                if editor.confirm_selection == ConfirmChoice::Cancel {
                    sel_style
                } else {
                    btn_style
                },
            ),
        ]);

        let buttons_p = Paragraph::new(buttons).alignment(Alignment::Center);

        let border = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(cs.primary))
            .title(Span::styled(
                " Unsaved Changes ",
                Style::default()
                    .fg(cs.accent)
                    .add_modifier(Modifier::BOLD),
            ));
        f.render_widget(border, area);
        f.render_widget(msg, inner[0]);
        f.render_widget(buttons_p, inner[1]);
    }

    // Add feed dialog
    if editor.adding_feed {
        let area = centered_rect(60, 11, f.size());
        f.render_widget(Clear, area);

        let inner = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2),
                Constraint::Length(2),
                Constraint::Length(2),
                Constraint::Min(0),
            ])
            .margin(1)
            .split(area);

        let border = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(cs.primary))
            .title(Span::styled(
                " Add Feed ",
                Style::default()
                    .fg(cs.accent)
                    .add_modifier(Modifier::BOLD),
            ));
        f.render_widget(border, area);

        let url_style = if editor.add_field_focus == 0 {
            Style::default().fg(cs.primary)
        } else {
            Style::default().fg(cs.text_secondary)
        };
        let cat_style = if editor.add_field_focus == 1 {
            Style::default().fg(cs.primary)
        } else {
            Style::default().fg(cs.text_secondary)
        };

        let url_display = if editor.add_field_focus == 0 {
            format!("{}█", &editor.add_url_buffer)
        } else {
            editor.add_url_buffer.clone()
        };
        let cat_display = if editor.add_field_focus == 1 {
            format!("{}█", &editor.add_category_buffer)
        } else {
            editor.add_category_buffer.clone()
        };

        let url_line = Paragraph::new(Line::from(vec![
            Span::styled("URL:      ", Style::default().fg(cs.text)),
            Span::styled(url_display, url_style),
        ]));

        let cat_line = Paragraph::new(Line::from(vec![
            Span::styled("Category: ", Style::default().fg(cs.text)),
            Span::styled(cat_display, cat_style),
        ]));

        let confirm_style = if editor.add_field_focus == 2 {
            Style::default()
                .fg(cs.primary)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
        } else {
            Style::default().fg(cs.text_secondary)
        };
        let confirm_line =
            Paragraph::new(Line::from(Span::styled("[Confirm]", confirm_style)))
                .alignment(Alignment::Center);

        f.render_widget(url_line, inner[0]);
        f.render_widget(cat_line, inner[1]);
        f.render_widget(confirm_line, inner[2]);
    }
}

fn centered_rect(percent_x: u16, height: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length((area.height.saturating_sub(height)) / 2),
            Constraint::Length(height),
            Constraint::Min(0),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
