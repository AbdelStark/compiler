use std::collections::HashSet;
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
    pub breakpoints: Vec<usize>,
    pub last_outcome: Option<VmOutcome>,
}

pub fn run_debugger(
    script: Vec<String>,
    env: &ExecutionEnv,
    headless: bool,
    initial_breakpoints: Vec<usize>,
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
        "Toggle Breakpoint (b)".to_string(),
        "Quit (q)".to_string(),
    ];

    let mut breakpoints: HashSet<usize> = initial_breakpoints.into_iter().collect();

    if headless {
        let mut sorted_breakpoints: Vec<usize> = breakpoints.iter().copied().collect();
        sorted_breakpoints.sort_unstable();
        return Ok(DebuggerReport {
            panes,
            controls,
            breakpoints: sorted_breakpoints,
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
    let mut logs: Vec<String> = vec![
        "Debugger ready. Controls: n/c/r/b/q".to_string(),
        "Use 'b' on current line to toggle breakpoint".to_string(),
    ];

    loop {
        let mut sorted_breakpoints: Vec<usize> = breakpoints.iter().copied().collect();
        sorted_breakpoints.sort_unstable();

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
                        let bp_marker = if breakpoints.contains(&idx) { "*" } else { " " };
                        if idx == vm.ip {
                            ListItem::new(format!(">{bp_marker} {idx:04}  {token}"))
                                .style(Style::default().add_modifier(Modifier::BOLD))
                        } else {
                            ListItem::new(format!(" {bp_marker} {idx:04}  {token}"))
                        }
                    })
                    .collect();

                let bp_line = if sorted_breakpoints.is_empty() {
                    "Breakpoints: none".to_string()
                } else {
                    format!(
                        "Breakpoints: {}",
                        sorted_breakpoints
                            .iter()
                            .map(|v| v.to_string())
                            .collect::<Vec<_>>()
                            .join(",")
                    )
                };

                let script_block = List::new(script_items).block(
                    Block::default()
                        .title("Script")
                        .borders(Borders::ALL)
                        .title_bottom(format!(
                            "Step: n  Continue: c  Reset: r  Toggle BP: b  Quit: q  |  {}",
                            bp_line
                        )),
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
                            continue_until_breakpoint(&mut vm, env, &breakpoints, &mut logs);
                        }
                    }
                    KeyCode::Char('r') => {
                        vm.reset();
                        logs.push("vm reset".to_string());
                    }
                    KeyCode::Char('b') => {
                        if breakpoints.contains(&vm.ip) {
                            breakpoints.remove(&vm.ip);
                            logs.push(format!("removed breakpoint at ip={}", vm.ip));
                        } else {
                            breakpoints.insert(vm.ip);
                            logs.push(format!("added breakpoint at ip={}", vm.ip));
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    let mut sorted_breakpoints: Vec<usize> = breakpoints.into_iter().collect();
    sorted_breakpoints.sort_unstable();

    let _ = disable_raw_mode();
    let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen);
    let _ = terminal.show_cursor();

    Ok(DebuggerReport {
        panes,
        controls,
        breakpoints: sorted_breakpoints,
        last_outcome: vm.result.clone(),
    })
}

fn continue_until_breakpoint(
    vm: &mut VMState,
    env: &ExecutionEnv,
    breakpoints: &HashSet<usize>,
    logs: &mut Vec<String>,
) {
    if breakpoints.contains(&vm.ip) {
        logs.push(format!("breakpoint hit at ip={}", vm.ip));
        return;
    }

    loop {
        if vm.halted {
            logs.push(format!("continue => {:?}", vm.result));
            break;
        }

        if let Err(err) = vm.step(env) {
            vm.halted = true;
            vm.result = Some(VmOutcome::RuntimeError(err.clone()));
            logs.push(format!("runtime_error: {err}"));
            break;
        }

        if let Some(last) = vm.telemetry.last() {
            logs.push(format!(
                "ip={} token={} status={:?} stack={:?}",
                last.ip, last.token, last.status, last.stack_after
            ));
        }

        if vm.halted {
            logs.push(format!("continue => {:?}", vm.result));
            break;
        }

        if breakpoints.contains(&vm.ip) {
            logs.push(format!("breakpoint hit at ip={}", vm.ip));
            break;
        }
    }
}
