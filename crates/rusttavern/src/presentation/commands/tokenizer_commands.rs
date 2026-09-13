use std::sync::Arc;

use crate::app::AppState;
use crate::presentation::commands::helpers::{log_command, map_command_error};
use crate::presentation::errors::CommandError;
use tt_application::dto::tokenization_dto::{
    OpenAiDecodeRequestDto, OpenAiDecodeResponseDto, OpenAiEncodeRequestDto,
    OpenAiEncodeResponseDto, OpenAiLogitBiasRequestDto, OpenAiLogitBiasResponseDto,
    OpenAiTokenCountBatchRequestDto, OpenAiTokenCountBatchResponseDto, OpenAiTokenCountRequestDto,
    OpenAiTokenCountResponseDto, OpenAiTokenPrefixCountRequestDto,
};

pub async fn count_openai_tokens(
    dto: OpenAiTokenCountRequestDto,
    state: Arc<AppState>,
) -> Result<OpenAiTokenCountResponseDto, CommandError> {
    log_command("count_openai_tokens");

    state
        .services
        .tokenization_service
        .count_openai_tokens(dto)
        .await
        .map_err(map_command_error("Failed to count OpenAI tokens"))
}

pub async fn count_openai_tokens_batch(
    dto: OpenAiTokenCountBatchRequestDto,
    state: Arc<AppState>,
) -> Result<OpenAiTokenCountBatchResponseDto, CommandError> {
    log_command("count_openai_tokens_batch");

    state
        .services
        .tokenization_service
        .count_openai_tokens_batch(dto)
        .await
        .map_err(map_command_error("Failed to count OpenAI tokens batch"))
}

pub async fn count_openai_token_prefixes(
    dto: OpenAiTokenPrefixCountRequestDto,
    state: Arc<AppState>,
) -> Result<OpenAiTokenCountBatchResponseDto, CommandError> {
    log_command("count_openai_token_prefixes");

    state
        .services
        .tokenization_service
        .count_openai_token_prefixes(dto)
        .await
        .map_err(map_command_error("Failed to count OpenAI token prefixes"))
}

pub async fn encode_openai_tokens(
    dto: OpenAiEncodeRequestDto,
    state: Arc<AppState>,
) -> Result<OpenAiEncodeResponseDto, CommandError> {
    log_command("encode_openai_tokens");

    state
        .services
        .tokenization_service
        .encode_openai_tokens(dto)
        .await
        .map_err(map_command_error("Failed to encode OpenAI tokens"))
}

pub async fn decode_openai_tokens(
    dto: OpenAiDecodeRequestDto,
    state: Arc<AppState>,
) -> Result<OpenAiDecodeResponseDto, CommandError> {
    log_command("decode_openai_tokens");

    state
        .services
        .tokenization_service
        .decode_openai_tokens(dto)
        .await
        .map_err(map_command_error("Failed to decode OpenAI tokens"))
}

pub async fn build_openai_logit_bias(
    dto: OpenAiLogitBiasRequestDto,
    state: Arc<AppState>,
) -> Result<OpenAiLogitBiasResponseDto, CommandError> {
    log_command("build_openai_logit_bias");

    state
        .services
        .tokenization_service
        .build_openai_logit_bias(dto)
        .await
        .map_err(map_command_error("Failed to build OpenAI logit bias"))
}
