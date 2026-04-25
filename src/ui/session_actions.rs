use crate::app::App;
use crate::domain::{CodingAgentProduct, SessionKind};
use crate::ui::state::Route;
use anyhow::{Result, anyhow};

pub fn start_selected_task_session(
    app: &mut App,
    product: CodingAgentProduct,
    kind: SessionKind,
    return_route: Route,
) -> Result<Option<String>> {
    let project = app
        .selected_project()
        .cloned()
        .ok_or_else(|| anyhow!("no project selected"))?;
    let task = app
        .selected_task()
        .cloned()
        .ok_or_else(|| anyhow!("no task selected"))?;
    let session = app.services.session_service.start_session(
        &project,
        &task,
        product.clone(),
        kind.clone(),
    )?;
    app.reload_sessions()?;
    app.reload_tasks()?;

    if matches!(kind, SessionKind::Live) {
        app.set_route(Route::LiveSession);
        app.set_status(format!("Session {}", session.session_id));
        Ok(Some(session.tmux_session_name))
    } else {
        app.set_route(return_route);
        app.set_status(format!(
            "Started {} {} session",
            product.label(),
            kind.label()
        ));
        Ok(None)
    }
}

pub fn attach_selected_task_session(
    app: &mut App,
    product: CodingAgentProduct,
    kind: SessionKind,
) -> Result<Option<String>> {
    let task = app
        .selected_task()
        .cloned()
        .ok_or_else(|| anyhow!("no task selected"))?;
    match app.services.task_service.find_session(&task, product, kind) {
        Ok(session) => {
            app.set_route(Route::LiveSession);
            app.set_status(format!("Session {}", session.session_id));
            Ok(Some(session.tmux_session_name))
        }
        Err(error) => {
            app.set_status(error.to_string());
            Ok(None)
        }
    }
}
