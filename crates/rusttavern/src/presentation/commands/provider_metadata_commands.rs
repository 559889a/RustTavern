use std::sync::Arc;

use serde_json::Value;
use crate::app::AppState;
use crate::presentation::commands::helpers::{log_command, map_command_error};
use crate::presentation::errors::CommandError;
use tt_application::dto::provider_metadata_dto::{
    ProviderModelProvidersRequestDto, SiliconFlowEmbeddingModelsRequestDto,
    WorkersAiModelsRequestDto,
};
use tt_ports::repositories::provider_metadata_repository::{
    NanoGptCredits, NanoGptModelProviders, OpenRouterCredits,
};

pub async fn get_openrouter_model_providers(
    dto: ProviderModelProvidersRequestDto,
    state: Arc<AppState>,
) -> Result<Vec<String>, CommandError> {
    log_command(format!("get_openrouter_model_providers {}", dto.model));

    state
        .services
        .provider_metadata_service
        .openrouter_model_providers(dto)
        .await
        .map_err(map_command_error(
            "Failed to get OpenRouter model providers",
        ))
}

pub async fn get_openrouter_credits(
    state: Arc<AppState>,
) -> Result<OpenRouterCredits, CommandError> {
    log_command("get_openrouter_credits");

    state
        .services
        .provider_metadata_service
        .openrouter_credits()
        .await
        .map_err(map_command_error("Failed to get OpenRouter credits"))
}

pub async fn get_nanogpt_model_providers(
    dto: ProviderModelProvidersRequestDto,
    state: Arc<AppState>,
) -> Result<NanoGptModelProviders, CommandError> {
    log_command(format!("get_nanogpt_model_providers {}", dto.model));

    state
        .services
        .provider_metadata_service
        .nanogpt_model_providers(dto)
        .await
        .map_err(map_command_error("Failed to get NanoGPT model providers"))
}

pub async fn get_nanogpt_credits(
    state: Arc<AppState>,
) -> Result<NanoGptCredits, CommandError> {
    log_command("get_nanogpt_credits");

    state
        .services
        .provider_metadata_service
        .nanogpt_credits()
        .await
        .map_err(map_command_error("Failed to get NanoGPT credits"))
}

pub async fn get_siliconflow_embedding_models(
    dto: SiliconFlowEmbeddingModelsRequestDto,
    state: Arc<AppState>,
) -> Result<Vec<Value>, CommandError> {
    log_command("get_siliconflow_embedding_models");

    state
        .services
        .provider_metadata_service
        .siliconflow_embedding_models(dto)
        .await
        .map_err(map_command_error(
            "Failed to get SiliconFlow embedding models",
        ))
}

pub async fn get_workers_ai_embedding_models(
    dto: WorkersAiModelsRequestDto,
    state: Arc<AppState>,
) -> Result<Vec<Value>, CommandError> {
    log_command("get_workers_ai_embedding_models");

    state
        .services
        .provider_metadata_service
        .workers_ai_embedding_models(dto)
        .await
        .map_err(map_command_error(
            "Failed to get Cloudflare Workers AI embedding models",
        ))
}

pub async fn get_workers_ai_multimodal_models(
    dto: WorkersAiModelsRequestDto,
    state: Arc<AppState>,
) -> Result<Vec<String>, CommandError> {
    log_command("get_workers_ai_multimodal_models");

    state
        .services
        .provider_metadata_service
        .workers_ai_multimodal_models(dto)
        .await
        .map_err(map_command_error(
            "Failed to get Cloudflare Workers AI multimodal models",
        ))
}
