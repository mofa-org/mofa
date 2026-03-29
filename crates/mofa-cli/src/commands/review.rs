//! Manual review management command implementation

use crate::CliError;
use crate::context::CliContext;
use crate::output::Table;
use mofa_kernel::hitl::{ReviewResponse, ReviewStatus};
use colored::Colorize;

/// List pending review requests
pub async fn list(ctx: &CliContext, execution_id: Option<&str>, all: bool) -> Result<(), CliError> {
    let reviews = ctx.review_store.list()?;
    let mut table = Table::new();
    table.set_header(vec!["ID", "Execution", "Node", "Type", "Status", "Created"]);

    for (id, review) in reviews {
        if !all && review.is_resolved() {
            continue;
        }
        if let Some(eid) = execution_id {
            if review.execution_id != eid {
                continue;
            }
        }

        let status_color = match review.status {
            ReviewStatus::Pending => review.status.to_string().yellow(),
            ReviewStatus::Approved => review.status.to_string().green(),
            ReviewStatus::Rejected => review.status.to_string().red(),
            ReviewStatus::Retrying => review.status.to_string().blue(),
            _ => review.status.to_string().white(),
        };

        table.add_row(vec![
            id.blue().to_string(),
            review.execution_id.cyan().to_string(),
            review.node_id.clone().unwrap_or_default(),
            format!("{:?}", review.review_type),
            status_color.to_string(),
            review.created_at.to_string(),
        ]);
    }

    if table.is_empty() {
        println!("{} No pending reviews found", "→".yellow());
    } else {
        println!("{}", table);
    }

    Ok(())
}

/// Handle a review response (Approve, Reject, Retry)
pub async fn respond(
    ctx: &CliContext,
    id: &str,
    response: ReviewResponse,
    status: ReviewStatus,
) -> Result<(), CliError> {
    let mut review = ctx.review_store.get(id)?.ok_or_else(|| {
        CliError::Other(format!("Review request '{}' not found", id))
    })?;

    review.status = status;
    review.response = Some(response);
    review.resolved_at = Some(chrono::Utc::now());
    
    ctx.review_store.save(id, &review)?;
    
    println!("{} Review '{}' updated to {:?}", "✓".green(), id, review.status);
    Ok(())
}
