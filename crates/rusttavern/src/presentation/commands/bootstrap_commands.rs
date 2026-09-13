use std::sync::Arc;

use crate::app::AppState;
use crate::presentation::commands::helpers::{log_command, map_command_error};
use crate::presentation::errors::CommandError;
use tt_application::dto::bootstrap_dto::BootstrapSnapshotDto;
use tt_application::dto::group_dto::GroupDto;

pub async fn get_bootstrap_snapshot(
    state: Arc<AppState>,
) -> Result<BootstrapSnapshotDto, CommandError> {
    log_command("get_bootstrap_snapshot");

    let settings_fut = async {
        state
            .services
            .settings_service
            .get_sillytavern_settings()
            .await
            .map_err(map_command_error(
                "Failed to load bootstrap settings snapshot",
            ))
    };

    let characters_fut = async {
        state
            .services
            .character_service
            .get_all_characters(true)
            .await
            .map_err(map_command_error(
                "Failed to load bootstrap characters snapshot",
            ))
    };

    let groups_fut = async {
        state
            .services
            .group_service
            .get_all_groups()
            .await
            .map(|groups| groups.into_iter().map(GroupDto::from).collect())
            .map_err(map_command_error(
                "Failed to load bootstrap groups snapshot",
            ))
    };

    let avatars_fut = async {
        state
            .services
            .avatar_service
            .get_avatars()
            .await
            .map_err(map_command_error(
                "Failed to load bootstrap avatars snapshot",
            ))
    };

    let secret_state_fut = async {
        state
            .services
            .secret_service
            .read_secret_state()
            .await
            .map_err(map_command_error(
                "Failed to load bootstrap secret state snapshot",
            ))
    };

    let (settings, characters, groups, avatars, secret_state) = tokio::try_join!(
        settings_fut,
        characters_fut,
        groups_fut,
        avatars_fut,
        secret_state_fut
    )?;

    Ok(BootstrapSnapshotDto {
        ios_policy: state.ios_policy.clone(),
        settings,
        characters,
        groups,
        avatars,
        secret_state,
    })
}

pub async fn backend_error_bridge_ready(
    state: Arc<AppState>,
) -> Result<Vec<String>, CommandError> {
    log_command("backend_error_bridge_ready");
    Ok(state.host.backend_errors.mark_bridge_ready_and_drain())
}

pub async fn wait_for_backend_ready(state: Arc<AppState>) -> Result<(), CommandError> {
    log_command("wait_for_backend_ready");
    state
        .host
        .backend_readiness
        .wait_ready()
        .await
        .map_err(|error| CommandError::InternalServerError(error.to_string()))
}
