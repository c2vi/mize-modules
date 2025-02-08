
use color_eyre::Result;
use crossterm::event::DisableMouseCapture;
use crossterm::event::EnableMouseCapture;
use crossterm::event::MouseEventKind;
use crossterm::Command as CrosstermCommand;
use crossterm::ExecutableCommand;
use ratatui::widgets::Cell;
use ratatui::widgets::Table;
use ratatui::widgets::Row;
use ratatui::widgets::TableState;
use ratatui::{
    buffer::{Buffer},
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
use std::{fs, os::unix::thread::JoinHandleExt, sync::atomic::Ordering, thread};

use crossterm::event::KeyModifiers;
use std::{f32::consts::PI, process::Command};
use std::path::PathBuf;
use std::time::Duration;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use mize::{error::IntoMizeResult, mize_err, Instance, MizeResult};
use std::fs::OpenOptions;
use std::io::Write;
use mize::MizeError;

use crate::{DevModule, DevModuleData, DevModuleEvent};

const TODO_HEADER_STYLE: Style = Style::new().fg(SLATE.c100).bg(BLUE.c800);
const NORMAL_ROW_BG: Color = SLATE.c950;
const ALT_ROW_BG_COLOR: Color = SLATE.c900;
const SELECTED_STYLE: Style = Style::new().bg(SLATE.c800).add_modifier(Modifier::BOLD);
const TEXT_FG_COLOR: Color = SLATE.c200;
const COMPLETED_TEXT_FG_COLOR: Color = GREEN.c500;


pub struct TuiState {
    modules_state: TableState,
    scroll_offset: u16,
    build_status: String,
}

pub struct Tui<'a> {
    should_exit: bool,
    dev_module: &'a mut DevModule,
}


impl Widget for &mut Tui<'_> {
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
impl Tui<'_> {

    pub fn new(dev_module: &mut DevModule) -> MizeResult<Tui<'_>> {
        Ok(Tui {
            should_exit: false, 
            dev_module,
        })
    }

    pub fn state(&mut self) -> &mut TuiState {
        self.dev_module.tui_state()
    }


    pub fn init_state(dev_module: &mut DevModule) {

        // if there is no tui state, initialize it
        if dev_module.tui_state.is_none() {
            dev_module.tui_state = Some( TuiState {
                scroll_offset: 0,
                modules_state: TableState::default(),
                build_status: "idle".to_string(),
            });
        }
    }


    pub fn run(&mut self) -> MizeResult<()> {
        // start ratatui
        color_eyre::install()?;
        let mut terminal = ratatui::init();
        let mut stdout = std::io::stdout();
        stdout.execute(EnableMouseCapture {});

        let tx = self.dev_module.event_tx.clone();
        let cancel_thread_one = Arc::new(AtomicBool::new(false));
        let cancel_thread_two = cancel_thread_one.clone();
        let handle = std::thread::spawn(move || {
            while !cancel_thread_two.load(Ordering::Acquire) {
                if event::poll(Duration::from_millis(250)).unwrap() {
                    let event = match event::read() {
                        Ok(val) => val,
                        Err(e) => continue,
                    };
                    if let Err(e) = tx.send(DevModuleEvent::Term(event)) {
                        println!("error sending Term event");
                    };
                }
            }
        });

        let dir_path = PathBuf::from(self.dev_module.instance.get("0/config/store_path")?.value_string()?).join("mize_dev_module");

        // event loop
        while !self.should_exit {

            terminal.draw(|frame| frame.render_widget(&mut *self, frame.area()))?;


            match self.dev_module.event_rx.recv()? {

                DevModuleEvent::Term(ev) => {
                    match ev {
                        Event::Key(key_event) => self.handle_key(key_event)?,
                        Event::Mouse(mouse_event) => {
                            match mouse_event.kind {
                                MouseEventKind::ScrollUp => {
                                    if self.state().scroll_offset > (u16::min_value() +3) {
                                        self.state().scroll_offset -= 3;
                                    }
                                }
                                MouseEventKind::ScrollDown => {
                                    if self.state().scroll_offset < (u16::max_value() -3) {
                                        self.state().scroll_offset += 3;
                                    }
                                }
                                _ => continue
                            }
                        }
                        _ => continue,
                    }
                },

                DevModuleEvent::BuildFinished(name) => {
                    match self.dev_module.state.get_mut(&name) {
                        None => {
                            self.dev_module.state.insert(name.clone(), "idle".to_owned());
                        },
                        Some(state) => {
                            *state = "idle".to_owned();
                        },
                    };
                },

                DevModuleEvent::BuildOutput((name, line)) => {
                    self.dev_module.outputs.get_mut(&name).ok_or(mize_err!("..."))?.push(line.clone() + "\n");
                    let mut file = OpenOptions::new()
                        .create(true)
                        .write(true)
                        .append(true)
                        .open(dir_path.as_path().join(format!("{}.log", name)))?;
                    write!(file, "{}\n", line);
                    //println!("output from {}: {}", name, line);
                },

                DevModuleEvent::RunBuild => {
                    self.dev_module.run_build()?;
                },
            }

        }

        // restore terminal
        ratatui::restore();
        stdout.execute(DisableMouseCapture {});

        cancel_thread_one.store(true, Ordering::Relaxed);
        


        Ok(())
    }


    /// Changes the status of the selected list item
    fn toggle_status(&mut self) {
        println!("toggling status..................");
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
            KeyCode::Char('q') | KeyCode::Esc => {
                self.dev_module.stop_dev_shells()?;
                self.should_exit = true;
            },

            KeyCode::Char('j') | KeyCode::Down => self.state().modules_state.select_next(),
            KeyCode::Char('k') | KeyCode::Up => self.state().modules_state.select_previous(),
            KeyCode::Char('g') | KeyCode::Home => self.state().modules_state.select_first(),
            KeyCode::Char('G') | KeyCode::End => self.state().modules_state.select_last(),

            KeyCode::Char('u') | KeyCode::End => {
                if self.state().scroll_offset > (u16::min_value() +3) {
                    self.state().scroll_offset -= 3;
                }
            },
            KeyCode::Char('i') | KeyCode::End => {
                if self.state().scroll_offset < (u16::max_value() -3) {
                    self.state().scroll_offset += 3;
                }
            },

            KeyCode::Char('r') | KeyCode::End => {
                thread::spawn(|| {
                });
                self.dev_module.run_build()?
            },

            // TODO: KeyCode::Char('a') | KeyCode::End => tui_add_buildable(&mut self.data),
            _ => {}
        };

        Ok(())
    }

    fn render_list(&mut self, area: Rect, buf: &mut Buffer) {

        let block = Block::new()
            .title(Line::raw("Modules").centered())
            .borders(Borders::TOP)
            .border_set(symbols::border::EMPTY)
            .border_style(TODO_HEADER_STYLE)
            .bg(NORMAL_ROW_BG);

        // Iterate through all elements in the `items` and stylize them.
        let items: Vec<Row> = self
            .dev_module.data
            .buildables
            .iter()
            .enumerate()
            .map(|(i, buildable)| {
                let color = alternate_colors(i);
                let default_status = "idle".to_owned();
                let status = self.dev_module.state.get(&buildable.name).unwrap_or(&default_status);
                let active = true;
                let active_string = match active {
                    true => "active".to_owned(),
                    false => "deactivated".to_owned(),
                };

                Row::new([Cell::from(buildable.name.clone()), Cell::from(status.clone()), Cell::from(active_string)]).bg(color)
            })
            .collect();

        let widths = [
            Constraint::Ratio(1, 1),
            Constraint::Ratio(1, 1),
            Constraint::Ratio(1, 1),
        ];

        let table = Table::new(items, widths)
            .block(block)
            .highlight_style(SELECTED_STYLE)
            .highlight_symbol(">")
            .highlight_spacing(HighlightSpacing::Always);

        // We need to disambiguate this trait method as both `Widget` and `StatefulWidget` share the
        // same method name `render`.
        StatefulWidget::render(table, area, buf, &mut self.state().modules_state);
    }

    fn render_selected_item(&mut self, area: Rect, buf: &mut Buffer) {
        // We get the info depending on the item's state.
        

        let info = match self.state().modules_state.selected() {
            None => "No Module selected".to_string(),
            Some(index) => {
                let buildable_name = self.dev_module.data.buildables.iter().nth(index).unwrap().name.clone();
                let default_output = Vec::new();
                let output = self.dev_module.outputs.get(&buildable_name).unwrap_or(&default_output);
                output.join("")
            },
        };

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
            .scroll((self.state().scroll_offset, 0))
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

