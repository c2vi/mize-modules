
use color_eyre::Result;
use ratatui::{
    buffer::Buffer,
    crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind},
    layout::{Constraint, Layout, Rect},
    style::{
        palette::tailwind::{BLUE, GREEN, SLATE},
        Color, Modifier, Style, Stylize,
    },
    symbols,
    text::Line,
    widgets::{
        Block, Borders, HighlightSpacing, List, ListItem, ListState, Padding, Paragraph,
        StatefulWidget, Widget, Wrap,
    },
    DefaultTerminal,
};
use crossterm::event::KeyModifiers;
use std::process::Command;
use mize::{error::IntoMizeResult, Instance, MizeResult};

use crate::DevModuleData;

const TODO_HEADER_STYLE: Style = Style::new().fg(SLATE.c100).bg(BLUE.c800);
const NORMAL_ROW_BG: Color = SLATE.c950;
const ALT_ROW_BG_COLOR: Color = SLATE.c900;
const SELECTED_STYLE: Style = Style::new().bg(SLATE.c800).add_modifier(Modifier::BOLD);
const TEXT_FG_COLOR: Color = SLATE.c200;
const COMPLETED_TEXT_FG_COLOR: Color = GREEN.c500;

pub fn run_tui(data: DevModuleData, instance: &Instance) -> MizeResult<()> {
    color_eyre::install()?;
    let terminal = ratatui::init();
    let app = App {
        instance: instance.clone(),
        should_exit: false,
        data,
        dev_shells: Vec::new(),
        modules_state: ListState::default(),
    };
    let app_result = app.run(terminal);
    ratatui::restore();
    app_result
}


struct App {
    instance: Instance,
    should_exit: bool,
    data: DevModuleData,
    dev_shells: Vec<Command>,
    modules_state: ListState
}


impl App {
    fn run(mut self, mut terminal: DefaultTerminal) -> MizeResult<()> {
        while !self.should_exit {

            terminal.draw(|frame| frame.render_widget(&mut self, frame.area()))?;

            if let Event::Key(key) = event::read()? {
                self.handle_key(key)?;
            };
        }
        Ok(())
    }

    fn handle_key(&mut self, key: KeyEvent) -> MizeResult<()> {
        if key.kind != KeyEventKind::Press {
            return Ok(());
        }

        // CTRL+c should also exit
        if key.code == KeyCode::Char('c') && key.modifiers == KeyModifiers::CONTROL {
            self.should_exit = true
        }

        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_exit = true,

            KeyCode::Char('j') | KeyCode::Down => self.modules_state.select_next(),
            KeyCode::Char('k') | KeyCode::Up => self.modules_state.select_previous(),
            KeyCode::Char('g') | KeyCode::Home => self.modules_state.select_first(),
            KeyCode::Char('G') | KeyCode::End => self.modules_state.select_last(),

            KeyCode::Char('r') | KeyCode::End => crate::run_build(&self.data, &self.instance)?,

            // TODO: KeyCode::Char('a') | KeyCode::End => tui_add_buildable(&mut self.data),
            _ => {}
        };

        Ok(())
    }

    /// Changes the status of the selected list item
    fn toggle_status(&mut self) {
        println!("toggling status..................");
    }
}

impl Widget for &mut App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let [header_area, main_area, footer_area] = Layout::vertical([
            Constraint::Length(2),
            Constraint::Fill(1),
            Constraint::Length(1),
        ])
        .areas(area);

        let [list_area, item_area] =
            Layout::vertical([Constraint::Fill(1), Constraint::Fill(1)]).areas(main_area);

        // the header
        Paragraph::new("mize dev module tui")
            .bold()
            .centered()
            .render(header_area, buf);


        // the footer
        Paragraph::new("Use ↓↑ to move, g/G to go top/bottom.")
            .centered()
            .render(footer_area, buf);


        self.render_list(list_area, buf);
        self.render_selected_item(item_area, buf);
    }
}

/// Rendering logic for the app
impl App {

    fn render_list(&mut self, area: Rect, buf: &mut Buffer) {
        let block = Block::new()
            .title(Line::raw("Modules").centered())
            .borders(Borders::TOP)
            .border_set(symbols::border::EMPTY)
            .border_style(TODO_HEADER_STYLE)
            .bg(NORMAL_ROW_BG);

        // Iterate through all elements in the `items` and stylize them.
        let items: Vec<ListItem> = self
            .data
            .buildables
            .iter()
            .enumerate()
            .map(|(i, buildable)| {
                let color = alternate_colors(i);
                ListItem::new(buildable.name.clone()).bg(color)
            })
            .collect();

        // Create a List from all list items and highlight the currently selected one
        let list = List::new(items)
            .block(block)
            .highlight_style(SELECTED_STYLE)
            .highlight_symbol(">")
            .highlight_spacing(HighlightSpacing::Always);

        // We need to disambiguate this trait method as both `Widget` and `StatefulWidget` share the
        // same method name `render`.
        StatefulWidget::render(list, area, buf, &mut self.modules_state);
    }

    fn render_selected_item(&self, area: Rect, buf: &mut Buffer) {
        // We get the info depending on the item's state.
        //
        let info = "module infoooooooooooooo".to_string();

        // We show the list item's info under the list in this paragraph
        let block = Block::new()
            .title(Line::raw("Module Info").centered())
            .borders(Borders::TOP)
            .border_set(symbols::border::EMPTY)
            .border_style(TODO_HEADER_STYLE)
            .bg(NORMAL_ROW_BG)
            .padding(Padding::horizontal(1));

        Paragraph::new(info)
            .block(block)
            .fg(TEXT_FG_COLOR)
            .wrap(Wrap { trim: false })
            .render(area, buf);
    }
}

const fn alternate_colors(i: usize) -> Color {
    if i % 2 == 0 {
        NORMAL_ROW_BG
    } else {
        ALT_ROW_BG_COLOR
    }
}

