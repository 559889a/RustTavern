use std::sync::Arc;

use crate::app::AppState;
use crate::presentation::errors::CommandError;

/// Read a frontend template file from bundled resources.
pub fn read_frontend_template(
    name: String,
    state: Arc<AppState>,
) -> Result<String, CommandError> {
    state
        .host
        .templates
        .read_frontend_template(&name)
        .map_err(CommandError::from)
}

/// Read a built-in extension template file from bundled resources.
pub fn read_frontend_extension_template(
    extension: String,
    name: String,
    state: Arc<AppState>,
) -> Result<String, CommandError> {
    state
        .host
        .templates
        .read_frontend_extension_template(&extension, &name)
        .map_err(CommandError::from)
}
