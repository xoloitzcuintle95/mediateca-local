// SPDX-License-Identifier: MIT

//! Avisos y estados que se muestran sobre la página.

use cosmic::widget;

use crate::app::{Message, ScanState};
use crate::fl;
use crate::model::ScanWarning;

use super::CatalogView;

pub(super) fn push_status<'a>(
    mut content: widget::Column<'a, Message, cosmic::Theme, cosmic::Renderer>,
    state: &CatalogView<'a>,
) -> widget::Column<'a, Message, cosmic::Theme, cosmic::Renderer> {
    if matches!(state.scan_state, ScanState::Running) {
        content = content.push(widget::text::body(fl!("scanning")));
    }

    if let ScanState::Failed(reason) = state.scan_state {
        content = content.push(
            widget::warning(fl!("scan-failed", reason = reason))
                .on_close(Message::DismissScanError),
        );
    }

    if let Some(warning) = state.warning {
        content =
            content.push(widget::warning(warning_text(warning)).on_close(Message::DismissWarning));
    }

    if state.deleted > 0 {
        content = content.push(widget::text::caption(fl!("deleted", count = state.deleted)));
    }

    content
}
fn warning_text(warning: &ScanWarning) -> String {
    match warning {
        ScanWarning::RootUnavailable { root, reason } => {
            fl!("warning-root-unavailable", root = root, reason = reason)
        }
        ScanWarning::Incomplete { errors } => fl!("warning-incomplete", errors = errors),
        ScanWarning::EmptyRoot => fl!("warning-empty-root"),
        ScanWarning::InvalidCatalogRoot { reason } => {
            fl!("warning-invalid-root", reason = reason)
        }
        ScanWarning::OpenFailed { reason } => fl!("warning-open-failed", reason = reason),
    }
}
