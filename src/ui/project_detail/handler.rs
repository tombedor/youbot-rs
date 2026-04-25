use crate::app::App;
use crate::domain::{CodingAgentProduct, SessionKind, TaskStatus};
use crate::ui::session_actions::{attach_selected_task_session, start_selected_task_session};
use crate::ui::state::Route;
use anyhow::{Result, anyhow};
use crossterm::event::{KeyCode, KeyEvent};

pub fn handle(app: &mut App, key: KeyEvent) -> Result<Option<String>> {
    if app.is_creating_task() {
        match key.code {
            KeyCode::Esc => app.cancel_task_creation(),
            KeyCode::Backspace => {
                if let Some(draft) = app.task_draft_mut() {
                    draft.pop();
                }
            }
            KeyCode::Enter => {
                let description = app.task_draft().trim().to_string();
                if description.is_empty() {
                    app.set_status("Task description cannot be empty");
                } else {
                    let project = app
                        .selected_project()
                        .cloned()
                        .ok_or_else(|| anyhow::anyhow!("no project selected"))?;
                    let task = app
                        .services
                        .task_service
                        .create_task(&project, description)?;
                    app.reload_tasks()?;
                    app.complete_task_creation(task.title);
                }
            }
            KeyCode::Char(ch) => {
                if let Some(draft) = app.task_draft_mut() {
                    draft.push(ch);
                }
            }
            _ => {}
        }
        return Ok(None);
    }

    if app.is_choosing_status() {
        match key.code {
            KeyCode::Esc => app.cancel_status_selection(),
            KeyCode::Char('1') => apply_status(app, TaskStatus::Todo)?,
            KeyCode::Char('2') => apply_status(app, TaskStatus::InProgress)?,
            KeyCode::Char('3') => apply_status(app, TaskStatus::Complete)?,
            KeyCode::Char('4') => apply_status(app, TaskStatus::WontDo)?,
            _ => {}
        }
        return Ok(None);
    }

    match key.code {
        KeyCode::Esc => app.set_route(Route::Home),
        KeyCode::Down => {
            if !app.tasks().is_empty() {
                app.set_selected_task_index((app.selected_task_index() + 1) % app.tasks().len());
            }
        }
        KeyCode::Up => {
            if !app.tasks().is_empty() {
                app.set_selected_task_index(if app.selected_task_index() == 0 {
                    app.tasks().len() - 1
                } else {
                    app.selected_task_index() - 1
                });
            }
        }
        KeyCode::Enter => {
            if app.selected_task().is_some() {
                app.set_route(Route::TaskDetail);
            }
        }
        KeyCode::Char('n') => {
            app.begin_task_creation();
        }
        KeyCode::Char('s') => {
            app.begin_status_selection();
        }
        KeyCode::Char('m') => {
            let project = app
                .selected_project()
                .cloned()
                .ok_or_else(|| anyhow!("no project selected"))?;
            let next = !project.config.auto_merge;
            app.services
                .project_service
                .set_auto_merge(&project.id, next)?;
            app.reload_projects()?;
            app.set_status(if next {
                "Project set to auto-merge"
            } else {
                "Project set to open PRs"
            });
        }
        KeyCode::Char('l') => {
            let session_name = start_selected_task_session(
                app,
                CodingAgentProduct::Codex,
                SessionKind::Live,
                Route::ProjectDetail,
            )?
            .expect("live session should always attach immediately");
            return Ok(Some(session_name));
        }
        KeyCode::Char('b') => {
            start_selected_task_session(
                app,
                CodingAgentProduct::Codex,
                SessionKind::Background,
                Route::ProjectDetail,
            )?;
        }
        KeyCode::Char('a') => {
            if let Some(session_name) = attach_selected_task_session(
                app,
                CodingAgentProduct::Codex,
                SessionKind::Background,
            )? {
                return Ok(Some(session_name));
            }
        }
        KeyCode::Char('c') => {
            let session_name = start_selected_task_session(
                app,
                CodingAgentProduct::ClaudeCode,
                SessionKind::Live,
                Route::ProjectDetail,
            )?
            .expect("live session should always attach immediately");
            return Ok(Some(session_name));
        }
        _ => {}
    }
    Ok(None)
}

fn apply_status(app: &mut App, status: TaskStatus) -> Result<()> {
    let project = app
        .selected_project()
        .cloned()
        .ok_or_else(|| anyhow!("no project selected"))?;
    let task = app
        .selected_task()
        .cloned()
        .ok_or_else(|| anyhow!("no task selected"))?;
    app.services
        .task_service
        .set_status(&project, &task.id, status.clone())?;
    app.reload_tasks()?;
    app.complete_status_selection(status);
    Ok(())
}
