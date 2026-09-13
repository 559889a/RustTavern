use std::sync::Arc;

use crate::app::AppState;
use crate::presentation::commands::helpers::{log_command, map_command_error};
use crate::presentation::errors::CommandError;
use tt_application::dto::character_dto::{
    BulkMergeCharacterCardDataDto, BulkMergeCharacterCardDataResultDto, CharacterChatDto,
    CharacterDto, CharacterLorebookConflictDto, CheckCharacterLorebookConflictDto,
    CreateCharacterDto, CreateCharacterWithAvatarResultDto, CreateWithAvatarDto,
    DeleteCharacterDto, DuplicateCharacterDto, ExportCharacterContentDto,
    ExportCharacterContentResultDto, ExportCharacterDto, GetCharacterChatsDto, ImportCharacterDto,
    MergeCharacterCardDataDto, RenameCharacterDto, ReplaceCharacterDto,
    ResolveCharacterLorebookConflictDto, ResolveCharacterLorebookConflictResultDto,
    UpdateAvatarDto, UpdateCharacterCardDataDto, UpdateCharacterDto,
};
use tt_domain::models::skill::{SkillScope, SkillScopeRetargetRequest};

const SKILL_SOURCE_KIND_CHARACTER: &str = "character";

pub async fn get_all_characters(
    shallow: bool,
    state: Arc<AppState>,
) -> Result<Vec<CharacterDto>, CommandError> {
    log_command(format!("get_all_characters (shallow: {})", shallow));

    state
        .services
        .character_service
        .get_all_characters(shallow)
        .await
        .map_err(map_command_error("Failed to get all characters"))
}

pub async fn get_character(
    name: String,
    state: Arc<AppState>,
) -> Result<CharacterDto, CommandError> {
    log_command(format!("get_character {}", name));

    state
        .services
        .character_service
        .get_character(&name)
        .await
        .map_err(map_command_error(format!(
            "Failed to get character {}",
            name
        )))
}

pub async fn create_character(
    dto: CreateCharacterDto,
    state: Arc<AppState>,
) -> Result<CharacterDto, CommandError> {
    log_command(format!("create_character {}", dto.name));

    state
        .services
        .character_service
        .create_character(dto)
        .await
        .map_err(map_command_error("Failed to create character"))
}

pub async fn create_character_with_avatar(
    dto: CreateWithAvatarDto,
    state: Arc<AppState>,
) -> Result<CreateCharacterWithAvatarResultDto, CommandError> {
    log_command(format!(
        "create_character_with_avatar {}",
        dto.character.name
    ));

    state
        .services
        .character_service
        .create_with_avatar(dto)
        .await
        .map_err(map_command_error("Failed to create character with avatar"))
}

pub async fn update_character(
    name: String,
    dto: UpdateCharacterDto,
    state: Arc<AppState>,
) -> Result<CharacterDto, CommandError> {
    log_command(format!("update_character {}", name));

    state
        .services
        .character_service
        .update_character(&name, dto)
        .await
        .map_err(map_command_error("Failed to update character"))
}

pub async fn update_character_card_data(
    name: String,
    dto: UpdateCharacterCardDataDto,
    state: Arc<AppState>,
) -> Result<CharacterDto, CommandError> {
    log_command(format!("update_character_card_data {}", name));

    state
        .services
        .character_service
        .update_character_card_data(&name, dto)
        .await
        .map_err(map_command_error("Failed to update character card data"))
}

pub async fn check_character_lorebook_conflict(
    dto: CheckCharacterLorebookConflictDto,
    state: Arc<AppState>,
) -> Result<CharacterLorebookConflictDto, CommandError> {
    log_command(format!("check_character_lorebook_conflict {}", dto.name));

    state
        .services
        .character_service
        .check_lorebook_conflict(dto)
        .await
        .map_err(map_command_error(
            "Failed to check character lorebook conflict",
        ))
}

pub async fn resolve_character_lorebook_conflict(
    dto: ResolveCharacterLorebookConflictDto,
    state: Arc<AppState>,
) -> Result<ResolveCharacterLorebookConflictResultDto, CommandError> {
    log_command(format!("resolve_character_lorebook_conflict {}", dto.name));

    state
        .services
        .character_service
        .resolve_lorebook_conflict(dto)
        .await
        .map_err(map_command_error(
            "Failed to resolve character lorebook conflict",
        ))
}

pub async fn merge_character_card_data(
    name: String,
    dto: MergeCharacterCardDataDto,
    state: Arc<AppState>,
) -> Result<CharacterDto, CommandError> {
    log_command(format!("merge_character_card_data {}", name));

    state
        .services
        .character_service
        .merge_character_card_data(&name, dto)
        .await
        .map_err(map_command_error("Failed to merge character card data"))
}

pub async fn bulk_merge_character_card_data(
    dto: BulkMergeCharacterCardDataDto,
    state: Arc<AppState>,
) -> Result<BulkMergeCharacterCardDataResultDto, CommandError> {
    log_command("bulk_merge_character_card_data");

    state
        .services
        .character_service
        .bulk_merge_character_card_data(dto)
        .await
        .map_err(map_command_error(
            "Failed to bulk merge character card data",
        ))
}

pub async fn delete_character(
    dto: DeleteCharacterDto,
    state: Arc<AppState>,
) -> Result<(), CommandError> {
    log_command(format!("delete_character {}", dto.name));

    let name = dto.name.clone();
    state
        .services
        .character_service
        .delete_character(dto)
        .await
        .map_err(map_command_error("Failed to delete character"))?;

    state
        .services
        .skill_service
        .delete_skills_for_source(
            SKILL_SOURCE_KIND_CHARACTER,
            &character_skill_source_id(&name),
        )
        .await
        .map_err(map_command_error(
            "Failed to delete Agent Skills linked to character",
        ))?;

    Ok(())
}

pub async fn rename_character(
    dto: RenameCharacterDto,
    state: Arc<AppState>,
) -> Result<CharacterDto, CommandError> {
    log_command(format!(
        "rename_character {} -> {}",
        dto.old_name, dto.new_name
    ));

    let old_character_id = dto.old_name.clone();
    let renamed = state
        .services
        .character_service
        .rename_character(dto)
        .await
        .map_err(map_command_error("Failed to rename character"))?;

    let new_character_id = character_id_from_avatar(&renamed.avatar)?;
    if old_character_id != new_character_id {
        state
            .services
            .skill_service
            .retarget_scope(SkillScopeRetargetRequest {
                from_scope: SkillScope::Character {
                    character_id: old_character_id,
                },
                to_scope: SkillScope::Character {
                    character_id: new_character_id,
                },
            })
            .await
            .map_err(map_command_error(
                "Failed to retarget Agent Skills linked to character",
            ))?;
    }

    Ok(renamed)
}

pub async fn duplicate_character(
    dto: DuplicateCharacterDto,
    state: Arc<AppState>,
) -> Result<CharacterDto, CommandError> {
    log_command(format!("duplicate_character {}", dto.name));

    state
        .services
        .character_service
        .duplicate_character(dto)
        .await
        .map_err(map_command_error("Failed to duplicate character"))
}

pub async fn import_character(
    dto: ImportCharacterDto,
    state: Arc<AppState>,
) -> Result<CharacterDto, CommandError> {
    log_command(format!("import_character from {}", dto.file_path));

    state
        .services
        .character_service
        .import_character(dto)
        .await
        .map_err(map_command_error("Failed to import character"))
}

pub async fn replace_character(
    dto: ReplaceCharacterDto,
    state: Arc<AppState>,
) -> Result<CharacterDto, CommandError> {
    log_command(format!(
        "replace_character {} from {}",
        dto.name, dto.file_path
    ));

    state
        .services
        .character_service
        .replace_character(dto)
        .await
        .map_err(map_command_error("Failed to replace character"))
}

pub async fn export_character(
    dto: ExportCharacterDto,
    state: Arc<AppState>,
) -> Result<(), CommandError> {
    log_command(format!(
        "export_character {} to {}",
        dto.name, dto.target_path
    ));

    state
        .services
        .character_service
        .export_character(dto)
        .await
        .map_err(map_command_error("Failed to export character"))
}

pub async fn export_character_content(
    dto: ExportCharacterContentDto,
    state: Arc<AppState>,
) -> Result<ExportCharacterContentResultDto, CommandError> {
    log_command(format!(
        "export_character_content {} format {}",
        dto.name, dto.format
    ));

    state
        .services
        .character_service
        .export_character_content(dto)
        .await
        .map_err(map_command_error("Failed to export character content"))
}

pub async fn update_avatar(
    dto: UpdateAvatarDto,
    state: Arc<AppState>,
) -> Result<(), CommandError> {
    log_command(format!("update_avatar for {}", dto.name));

    state
        .services
        .character_service
        .update_avatar(dto)
        .await
        .map_err(map_command_error("Failed to update avatar"))
}

pub async fn get_character_chats_by_id(
    dto: GetCharacterChatsDto,
    state: Arc<AppState>,
) -> Result<Vec<CharacterChatDto>, CommandError> {
    log_command(format!("get_character_chats_by_id for {}", dto.name));

    state
        .services
        .character_service
        .get_character_chats(dto)
        .await
        .map_err(map_command_error("Failed to get character chats"))
}

pub async fn clear_character_cache(
    state: Arc<AppState>,
) -> Result<(), CommandError> {
    log_command("clear_character_cache");

    state
        .services
        .character_service
        .clear_cache()
        .await
        .map_err(map_command_error("Failed to clear character cache"))
}

fn character_skill_source_id(name: &str) -> String {
    format!("character:{}", name.trim())
}

fn character_id_from_avatar(avatar: &str) -> Result<String, CommandError> {
    let file_name = avatar
        .trim()
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or_default()
        .trim();
    let id = file_name
        .rsplit_once('.')
        .map(|(stem, _)| stem)
        .unwrap_or(file_name)
        .trim();
    if id.is_empty() {
        return Err(CommandError::BadRequest(
            "Character avatar did not resolve to a character id".to_string(),
        ));
    }
    Ok(id.to_string())
}
