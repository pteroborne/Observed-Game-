//! Re-export of the shared widget layer.
//!
//! The implementation moved to `crates/observed_ui` in Arc R, when the
//! composition studio became a second consumer and `agents.md`'s "generalise
//! only after the prototype works" condition was finally met.
//!
//! This shim keeps every screen's `widgets::…` path working, so the extraction
//! touched one file here instead of thirteen. That is deliberate: a move that
//! also rewrites its callers is two changes reviewed as one.

pub(crate) use observed_ui::{
    FocusScope, FocusScopeId, FocusTarget, FrontendWidgetsPlugin, UiFeedback, UiInputCapture,
    WidgetId, WidgetLabel, WidgetSpec, activation_enabled, focus_scope, spawn_button,
};
