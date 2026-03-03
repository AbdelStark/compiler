use std::time::Duration;

use crossterm::event::{self, Event, KeyCode};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};
use ratatui::Terminal;

use crate::runtime::env::ExecutionEnv;
use crate::runtime::error::{RuntimeError, RuntimeErrorCode};
use crate::runtime::vm::{VMState, VmOutcome};

#[derive(Debug, Clone)]
pub struct DebuggerReport {
    pub panes: Vec<String>,
    pub controls: Vec<String>,
    pub last_outcome: Option<VmOutcome>,
}

pub fn run_debugger(
    script: Vec<String>,
    env: &ExecutionEnv,
    headless: bool,
) -> Result<DebuggerReport, RuntimeError> {
    let panes = vec![
        "Script".to_string(),
        "Main Stack / Alt Stack".to_string(),
        "Telemetry Logs".to_string(),
    ];
    let controls = vec![
        "Step Over (n)".to_string(),
        "Continue (c)".to_string(),
        "Reset (r)".to_string(),
        "Quit (q)".to_string(),
    ];

    if headless {
        return Ok(DebuggerReport {
            panes,
            controls,
            last_outcome: None,
        });
    }

    enable_raw_mode().map_err(|err| {
        RuntimeError::new(
            RuntimeErrorCode::Debugger,
            format!("failed enabling raw mode: {err}"),
        )
    })?;

    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen).map_err(|err| {
        let _ = disable_raw_mode();
        RuntimeError::new(
            RuntimeErrorCode::Debugger,
            format!("failed entering alternate screen: {err}"),
        )
    })?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).map_err(|err| {
        let _ = disable_raw_mode();
        RuntimeError::new(
            RuntimeErrorCode::Debugger,
            format!("failed initializing terminal: {err}"),
        )
    })?;

    let mut vm = VMState::new(script);
    let mut logs: Vec<String> = vec!["Debugger ready. Controls: n/c/r/q".to_string()];

    loop {
        terminal
            .draw(|frame| {
                let root = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
                    .split(frame.area());

                let right = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
                    .split(root[1]);

                let script_items: Vec<ListItem> = vm
                    .script
                    .iter()
                    .enumerate()
                    .map(|(idx, token)| {
                        if idx == vm.ip {
                            ListItem::new(format!("> {idx:04}  {token}"))
                                .style(Style::default().add_modifier(Modifier::BOLD))
                        } else {
                            ListItem::new(format!("  {idx:04}  {token}"))
                        }
                    })
                    .collect();

                let script_block = List::new(script_items).block(
                    Block::default()
                        .title("Script")
                        .borders(Borders::ALL)
                        .title_bottom("Step Over: n  Continue: c  Reset: r  Quit: q"),
                );
                frame.render_widget(script_block, root[0]);

                let main_stack = vm
                    .stack
                    .snapshot_main()
                    .iter()
                    .enumerate()
                    .map(|(idx, value)| format!("{idx:03}: {}", value.display_compact()))
                    .collect::<Vec<_>>()
                    .join("\n");
                let alt_stack = vm
                    .stack
                    .snapshot_alt()
                    .iter()
                    .enumerate()
                    .map(|(idx, value)| format!("{idx:03}: {}", value.display_compact()))
                    .collect::<Vec<_>>()
                    .join("\n");

                let stack_widget = Paragraph::new(format!(
                    "Main Stack:\n{}\n\nAlt Stack:\n{}",
                    if main_stack.is_empty() {
                        "(empty)"
                    } else {
                        &main_stack
                    },
                    if alt_stack.is_empty() {
                        "(empty)"
                    } else {
                        &alt_stack
                    }
                ))
                .block(
                    Block::default()
                        .title("Main Stack / Alt Stack")
                        .borders(Borders::ALL),
                )
                .wrap(Wrap { trim: true });
                frame.render_widget(stack_widget, right[0]);

                let max_tail = 100usize;
                let logs_view = if logs.len() > max_tail {
                    logs[logs.len() - max_tail..].join("\n")
                } else {
                    logs.join("\n")
                };
                let logs_widget = Paragraph::new(logs_view)
                    .block(
                        Block::default()
                            .title("Telemetry Logs")
                            .borders(Borders::ALL),
                    )
                    .wrap(Wrap { trim: false });
                frame.render_widget(logs_widget, right[1]);
            })
            .map_err(|err| {
                RuntimeError::new(RuntimeErrorCode::Debugger, format!("draw error: {err}"))
            })?;

        if event::poll(Duration::from_millis(250)).map_err(|err| {
            RuntimeError::new(
                RuntimeErrorCode::Debugger,
                format!("event poll error: {err}"),
            )
        })? {
            if let Event::Key(key) = event::read().map_err(|err| {
                RuntimeError::new(
                    RuntimeErrorCode::Debugger,
                    format!("event read error: {err}"),
                )
            })? {
                match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Char('n') => {
                        if vm.halted {
                            logs.push("step ignored: VM already halted".to_string());
                        } else if let Err(err) = vm.step(env) {
                            vm.halted = true;
                            vm.result = Some(VmOutcome::RuntimeError(err.clone()));
                            logs.push(format!("runtime_error: {err}"));
                        } else if let Some(last) = vm.telemetry.last() {
                            logs.push(format!(
                                "ip={} token={} status={:?} stack={:?}",
                                last.ip, last.token, last.status, last.stack_after
                            ));
                        }
                    }
                    KeyCode::Char('c') => {
                        if vm.halted {
                            logs.push("continue ignored: VM already halted".to_string());
                        } else {
                            let run = vm.run(env);
                            logs.push(format!("continue => {:?}", run.outcome));
                        }
                    }
                    KeyCode::Char('r') => {
                        vm.reset();
                        logs.push("vm reset".to_string());
                    }
                    _ => {}
                }
            }
        }
    }

    let _ = disable_raw_mode();
    let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen);
    let _ = terminal.show_cursor();

    Ok(DebuggerReport {
        panes,
        controls,
        last_outcome: vm.result.clone(),
    })
}
