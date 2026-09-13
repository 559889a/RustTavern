//! Command registry construction.
//!
//! Replaces the old tauri::generate_handler! table. Every command is
//! registered by name through the register! / register_sync! macros;
//! argument names must match the command signature (state last).

use crate::register;
use crate::register_sync;
use crate::server::dispatch::CommandRegistry;

pub fn build_registry() -> CommandRegistry {
    let mut registry = CommandRegistry::new();

    // agent_commands
    register!(registry, super::agent_commands::start_agent_run, state: dto);
    register!(registry, super::agent_commands::prepare_agent_prompt_assembly, state: dto);
    register!(registry, super::agent_commands::build_agent_current_model_connection_snapshot, state: dto);
    register!(registry, super::agent_commands::apply_agent_current_model_connection_snapshot, state: dto);
    register!(registry, super::agent_commands::list_agent_profiles, state:);
    register!(registry, super::agent_commands::list_agent_tools, state:);
    register!(registry, super::agent_commands::resolve_agent_system_prompt, state: dto);
    register!(registry, super::agent_commands::load_agent_profile, state: dto);
    register!(registry, super::agent_commands::diagnose_agent_profile, state: dto);
    register!(registry, super::agent_commands::save_agent_profile, state: dto);
    register!(registry, super::agent_commands::delete_agent_profile, state: dto);
    register!(registry, super::agent_commands::repair_agent_profile_file, state: dto);
    register!(registry, super::agent_commands::retarget_agent_profile_preset_refs, state: dto);
    register!(registry, super::agent_commands::cancel_agent_run, state: dto);
    register!(registry, super::agent_commands::submit_agent_run_guidance, state: dto);
    register!(registry, super::agent_commands::list_agent_runs, state: dto);
    register!(registry, super::agent_commands::plan_agent_run_prune, state: dto);
    register!(registry, super::agent_commands::apply_agent_run_prune, state: dto);
    register!(registry, super::agent_commands::read_agent_run_events, state: dto);
    register!(registry, super::agent_commands::read_agent_workspace_file, state: dto);
    register!(registry, super::agent_commands::read_agent_model_turn, state: dto);
    register!(registry, super::agent_commands::read_agent_prompt_assembly_request, state: dto);
    register!(registry, super::agent_commands::resolve_agent_chat_commit, state: dto);
    register!(registry, super::agent_commands::resolve_agent_prompt_assembly, state: dto);
    register!(registry, super::agent_commands::resolve_agent_persistent_state_metadata_update, state: dto);
    register!(registry, super::agent_commands::prune_agent_chat_persistent_states, state: dto);
    // asset_commands
    register!(registry, super::asset_commands::get_assets_library, state:);
    register!(registry, super::asset_commands::download_asset, state: url, category, filename);
    register!(registry, super::asset_commands::delete_asset, state: category, filename);
    register!(registry, super::asset_commands::get_character_assets, state: name, category);
    // avatar_commands
    register!(registry, super::avatar_commands::get_avatars, state:);
    register!(registry, super::avatar_commands::delete_avatar, state: avatar);
    register!(registry, super::avatar_commands::upload_avatar, state: file_path, overwrite_name, crop);
    // background_commands
    register!(registry, super::background_commands::get_all_backgrounds, state:);
    register!(registry, super::background_commands::get_all_background_metadata, state: prefix);
    register!(registry, super::background_commands::delete_background, state: dto);
    register!(registry, super::background_commands::rename_background, state: dto);
    register!(registry, super::background_commands::upload_background, state: filename, data);
    register!(registry, super::background_commands::upload_background_from_path, state: filename, file_path);
    // bootstrap_commands
    register!(registry, super::bootstrap_commands::get_bootstrap_snapshot, state:);
    register!(registry, super::bootstrap_commands::backend_error_bridge_ready, state:);
    register!(registry, super::bootstrap_commands::wait_for_backend_ready, state:);
    // bridge
    register_sync!(registry, super::bridge::get_version, );
    register_sync!(registry, super::bridge::get_client_version, );
    register_sync!(registry, super::bridge::is_ready, );
    // character_commands
    register!(registry, super::character_commands::get_all_characters, state: shallow);
    register!(registry, super::character_commands::get_character, state: name);
    register!(registry, super::character_commands::create_character, state: dto);
    register!(registry, super::character_commands::create_character_with_avatar, state: dto);
    register!(registry, super::character_commands::update_character, state: name, dto);
    register!(registry, super::character_commands::update_character_card_data, state: name, dto);
    register!(registry, super::character_commands::check_character_lorebook_conflict, state: dto);
    register!(registry, super::character_commands::resolve_character_lorebook_conflict, state: dto);
    register!(registry, super::character_commands::merge_character_card_data, state: name, dto);
    register!(registry, super::character_commands::bulk_merge_character_card_data, state: dto);
    register!(registry, super::character_commands::delete_character, state: dto);
    register!(registry, super::character_commands::rename_character, state: dto);
    register!(registry, super::character_commands::duplicate_character, state: dto);
    register!(registry, super::character_commands::import_character, state: dto);
    register!(registry, super::character_commands::replace_character, state: dto);
    register!(registry, super::character_commands::export_character, state: dto);
    register!(registry, super::character_commands::export_character_content, state: dto);
    register!(registry, super::character_commands::update_avatar, state: dto);
    register!(registry, super::character_commands::get_character_chats_by_id, state: dto);
    register!(registry, super::character_commands::clear_character_cache, state:);
    // chat_api_commands
    register!(registry, super::chat_api_commands::get_character_chat_summary, state: character_name, file_name, include_metadata);
    register!(registry, super::chat_api_commands::get_character_chat_metadata, state: character_name, file_name);
    register!(registry, super::chat_api_commands::set_character_chat_metadata_extension, state: character_name, file_name, namespace, value);
    register!(registry, super::chat_api_commands::get_character_chat_store_json, state: character_name, file_name, namespace, key);
    register!(registry, super::chat_api_commands::set_character_chat_store_json, state: character_name, file_name, namespace, key, value);
    register!(registry, super::chat_api_commands::update_character_chat_store_json, state: character_name, file_name, namespace, key, value);
    register!(registry, super::chat_api_commands::rename_character_chat_store_key, state: character_name, file_name, namespace, key, new_key);
    register!(registry, super::chat_api_commands::delete_character_chat_store_json, state: character_name, file_name, namespace, key);
    register!(registry, super::chat_api_commands::list_character_chat_store_keys, state: character_name, file_name, namespace);
    register!(registry, super::chat_api_commands::find_last_character_chat_message, state: character_name, file_name, query);
    register!(registry, super::chat_api_commands::search_character_chat_messages, state: character_name, file_name, query);
    // chat_commands
    register!(registry, super::chat_commands::get_all_chats, state:);
    register!(registry, super::chat_commands::chat_history_generation_started, state: locator);
    register!(registry, super::chat_commands::chat_history_generation_finished, state: locator);
    register!(registry, super::chat_commands::get_chat, state: character_name, file_name);
    register!(registry, super::chat_commands::get_character_chats, state: character_name);
    register!(registry, super::chat_commands::create_chat, state: dto);
    register!(registry, super::chat_commands::add_message, state: dto);
    register!(registry, super::chat_commands::rename_chat, state: dto);
    register!(registry, super::chat_commands::delete_chat, state: character_name, file_name);
    register!(registry, super::chat_commands::search_chats, state: query, character_filter);
    register!(registry, super::chat_commands::list_chat_summaries, state: character_filter, include_metadata);
    register!(registry, super::chat_commands::list_recent_chat_summaries, state: character_filter, include_metadata, max_entries, pinned);
    register!(registry, super::chat_commands::import_chat, state: dto);
    register!(registry, super::chat_commands::export_chat, state: dto);
    register!(registry, super::chat_commands::backup_chat, state: character_name, file_name);
    register!(registry, super::chat_commands::list_chat_backups, state:);
    register!(registry, super::chat_commands::materialize_chat_backup, state: name);
    register!(registry, super::chat_commands::discard_chat_backup_materialization, state: path);
    register!(registry, super::chat_commands::restore_character_chat_backup, state: dto);
    register!(registry, super::chat_commands::delete_chat_backup, state: name);
    register!(registry, super::chat_commands::clear_chat_cache, state:);
    register!(registry, super::chat_commands::get_chat_payload_path, state: character_name, file_name, allow_not_found);
    register!(registry, super::chat_commands::get_chat_payload_tail, state: character_name, file_name, max_lines, allow_not_found);
    register!(registry, super::chat_commands::get_chat_payload_before, state: character_name, file_name, cursor, max_lines);
    register!(registry, super::chat_commands::get_chat_payload_before_pages, state: character_name, file_name, cursor, max_lines, max_pages);
    register!(registry, super::chat_commands::import_character_chats, state: dto);
    // chat_completion_commands
    register!(registry, super::chat_completion_commands::get_chat_completions_status, state: dto);
    register!(registry, super::chat_completion_commands::generate_chat_completion, state: dto, request_id);
    register!(registry, super::chat_completion_commands::start_chat_completion_stream, state: stream_id, dto);
    register!(registry, super::chat_completion_commands::cancel_chat_completion_stream, state: stream_id);
    register!(registry, super::chat_completion_commands::cancel_chat_completion_generation, state: request_id);
    // chat_payload_commit_commands
    register!(registry, super::chat_payload_commit_commands::begin_chat_commit, state: target, force, baseline);
    register!(registry, super::chat_payload_commit_commands::append_chat_commit_chunk, state: session_id, offset, data);
    register!(registry, super::chat_payload_commit_commands::finish_chat_commit, state: session_id, expected_size, commit_reason);
    register!(registry, super::chat_payload_commit_commands::abort_chat_commit, state: session_id);
    // content_commands
    register!(registry, super::content_commands::initialize_default_content, state:);
    register!(registry, super::content_commands::is_default_content_initialized, state:);
    register!(registry, super::content_commands::download_external_import_url, state: url);
    // data_archive_commands
    register!(registry, super::data_archive_commands::start_import_data_archive, state: archive_path, archive_is_temporary);
    register_sync!(registry, super::data_archive_commands::start_export_data_archive, state:);
    register_sync!(registry, super::data_archive_commands::prepare_data_archive_import_target_path, state:);
    register_sync!(registry, super::data_archive_commands::get_data_archive_job_status, state: job_id);
    register_sync!(registry, super::data_archive_commands::cancel_data_archive_job, state: job_id);
    register!(registry, super::data_archive_commands::save_export_data_archive, state: job_id);
    register_sync!(registry, super::data_archive_commands::cleanup_export_data_archive, state: job_id);
    register_sync!(registry, super::data_archive_commands::finalize_export_data_archive_delivery, state: job_id, saved_path);
    register!(registry, super::data_archive_commands::export_user_backup_archive, state: handle, include_secrets);
    register!(registry, super::data_archive_commands::save_user_backup_archive, state: archive_path, file_name);
    register_sync!(registry, super::data_archive_commands::cleanup_user_backup_archive, state: archive_path);
    // dev_logging_commands
    register!(registry, super::dev_logging_commands::devlog_append_frontend_logs, entries);
    register!(registry, super::dev_logging_commands::devlog_set_backend_log_stream_enabled, state: enabled);
    register!(registry, super::dev_logging_commands::devlog_get_backend_log_tail, state: limit);
    register!(registry, super::dev_logging_commands::devlog_set_llm_api_log_stream_enabled, state: enabled);
    register!(registry, super::dev_logging_commands::devlog_get_llm_api_log_index, state: limit);
    register!(registry, super::dev_logging_commands::devlog_get_llm_api_log_preview, state: id);
    register!(registry, super::dev_logging_commands::devlog_get_llm_api_log_raw, state: id);
    register!(registry, super::dev_logging_commands::devlog_export_bundle, state: frontend_entries);
    // extension_commands
    register!(registry, super::extension_commands::get_extensions, state:);
    register!(registry, super::extension_commands::install_extension, state: url, global, branch);
    register!(registry, super::extension_commands::update_extension, state: extension_name, global);
    register!(registry, super::extension_commands::delete_extension, state: extension_name, global);
    register!(registry, super::extension_commands::get_extension_version, state: extension_name, global);
    register!(registry, super::extension_commands::get_extension_branches, state: extension_name, global);
    register!(registry, super::extension_commands::switch_extension_branch, state: extension_name, branch, global);
    register!(registry, super::extension_commands::move_extension, state: extension_name, source, destination);
    // extension_store_commands
    register!(registry, super::extension_store_commands::get_extension_store_json, state: namespace, key, table);
    register!(registry, super::extension_store_commands::try_get_extension_store_json, state: namespace, key, table);
    register!(registry, super::extension_store_commands::set_extension_store_json, state: namespace, key, value, table);
    register!(registry, super::extension_store_commands::update_extension_store_json, state: namespace, key, value, table);
    register!(registry, super::extension_store_commands::rename_extension_store_key, state: namespace, key, new_key, table);
    register!(registry, super::extension_store_commands::delete_extension_store_json, state: namespace, key, table);
    register!(registry, super::extension_store_commands::list_extension_store_keys, state: namespace, table);
    register!(registry, super::extension_store_commands::list_extension_store_tables, state: namespace);
    register!(registry, super::extension_store_commands::delete_extension_store_table, state: namespace, table);
    register!(registry, super::extension_store_commands::get_extension_store_blob, state: namespace, key, table);
    register!(registry, super::extension_store_commands::set_extension_store_blob, state: namespace, key, data_base64, table);
    register!(registry, super::extension_store_commands::delete_extension_store_blob, state: namespace, key, table);
    register!(registry, super::extension_store_commands::list_extension_store_blob_keys, state: namespace, table);
    // file_commands
    register!(registry, super::file_commands::sanitize_filename, file_name);
    register!(registry, super::file_commands::upload_user_file, state: name, data_base64);
    register!(registry, super::file_commands::delete_user_file, state: path);
    register!(registry, super::file_commands::verify_user_files, state: urls);
    // group_chat_api_commands
    register!(registry, super::group_chat_api_commands::get_group_chat_summary, state: chat_id, include_metadata);
    register!(registry, super::group_chat_api_commands::get_group_chat_metadata, state: chat_id);
    register!(registry, super::group_chat_api_commands::set_group_chat_metadata_extension, state: chat_id, namespace, value);
    register!(registry, super::group_chat_api_commands::get_group_chat_store_json, state: chat_id, namespace, key);
    register!(registry, super::group_chat_api_commands::set_group_chat_store_json, state: chat_id, namespace, key, value);
    register!(registry, super::group_chat_api_commands::update_group_chat_store_json, state: chat_id, namespace, key, value);
    register!(registry, super::group_chat_api_commands::rename_group_chat_store_key, state: chat_id, namespace, key, new_key);
    register!(registry, super::group_chat_api_commands::delete_group_chat_store_json, state: chat_id, namespace, key);
    register!(registry, super::group_chat_api_commands::list_group_chat_store_keys, state: chat_id, namespace);
    register!(registry, super::group_chat_api_commands::find_last_group_chat_message, state: chat_id, query);
    register!(registry, super::group_chat_api_commands::search_group_chat_messages, state: chat_id, query);
    // group_chat_commands
    register!(registry, super::group_chat_commands::list_group_chat_summaries, state: chat_ids, include_metadata);
    register!(registry, super::group_chat_commands::list_recent_group_chat_summaries, state: chat_ids, include_metadata, max_entries, pinned);
    register!(registry, super::group_chat_commands::search_group_chats, state: query, chat_ids);
    register!(registry, super::group_chat_commands::get_group_chat_path, state: id, allow_not_found);
    register!(registry, super::group_chat_commands::get_group_chat_payload_tail, state: id, max_lines, allow_not_found);
    register!(registry, super::group_chat_commands::get_group_chat_payload_before, state: id, cursor, max_lines);
    register!(registry, super::group_chat_commands::get_group_chat_payload_before_pages, state: id, cursor, max_lines, max_pages);
    register!(registry, super::group_chat_commands::delete_group_chat, state: dto);
    register!(registry, super::group_chat_commands::rename_group_chat, state: dto);
    register!(registry, super::group_chat_commands::import_group_chat_payload, state: dto);
    register!(registry, super::group_chat_commands::restore_group_chat_backup, state: dto);
    // group_commands
    register!(registry, super::group_commands::get_all_groups, state:);
    register!(registry, super::group_commands::get_group, state: id);
    register!(registry, super::group_commands::create_group, state: dto);
    register!(registry, super::group_commands::update_group, state: dto);
    register!(registry, super::group_commands::delete_group, state: dto);
    register!(registry, super::group_commands::get_group_chat_paths, state:);
    register!(registry, super::group_commands::clear_group_cache, state:);
    // image_commands
    register!(registry, super::image_commands::upload_user_image, state: image_base64, format, filename, ch_name);
    register!(registry, super::image_commands::list_user_images, state: folder, sort_field, sort_order, media_type);
    register!(registry, super::image_commands::list_user_image_folders, state:);
    register!(registry, super::image_commands::delete_user_image, state: path);
    // image_metadata_commands
    register!(registry, super::image_metadata_commands::get_background_folders, state:);
    register!(registry, super::image_metadata_commands::create_image_metadata_folder, state: dto);
    register!(registry, super::image_metadata_commands::update_image_metadata_folder, state: dto);
    register!(registry, super::image_metadata_commands::delete_image_metadata_folder, state: dto);
    register!(registry, super::image_metadata_commands::set_image_metadata_folder_thumbnails, state: dto);
    register!(registry, super::image_metadata_commands::assign_images_to_metadata_folder, state: dto);
    register!(registry, super::image_metadata_commands::unassign_images_from_metadata_folder, state: dto);
    // lan_sync_commands
    register!(registry, super::lan_sync_commands::lan_sync_get_status, state:);
    register!(registry, super::lan_sync_commands::lan_sync_start_server, state:);
    register!(registry, super::lan_sync_commands::lan_sync_stop_server, state:);
    register!(registry, super::lan_sync_commands::lan_sync_enable_pairing, state: address);
    register!(registry, super::lan_sync_commands::lan_sync_get_pairing_info, state: address);
    register!(registry, super::lan_sync_commands::lan_sync_request_pairing, state: pair_uri);
    register!(registry, super::lan_sync_commands::lan_sync_confirm_pairing, state: request_id, accept);
    register!(registry, super::lan_sync_commands::lan_sync_list_devices, state:);
    register!(registry, super::lan_sync_commands::lan_sync_remove_device, state: device_id);
    register!(registry, super::lan_sync_commands::lan_sync_sync_from_device, state: device_id, options);
    register!(registry, super::lan_sync_commands::lan_sync_push_to_device, state: device_id, options);
    register!(registry, super::lan_sync_commands::lan_sync_set_sync_mode, state: mode, persist);
    register!(registry, super::lan_sync_commands::lan_sync_clear_sync_mode_override, state:);
    register!(registry, super::lan_sync_commands::lan_sync_set_overwrite_policy, state: overwrite_policy);
    // llm_connection_commands
    register!(registry, super::llm_connection_commands::list_llm_connections, state:);
    register!(registry, super::llm_connection_commands::load_llm_connection, state: dto);
    register!(registry, super::llm_connection_commands::save_llm_connection, state: dto);
    register!(registry, super::llm_connection_commands::delete_llm_connection, state: dto);
    // native_regex_commands
    register!(registry, super::native_regex_commands::apply_native_regex_batch, state: dto);
    // preset_commands
    register!(registry, super::preset_commands::save_preset, state: dto);
    register!(registry, super::preset_commands::delete_preset, state: dto);
    register!(registry, super::preset_commands::restore_preset, state: dto);
    register!(registry, super::preset_commands::save_openai_preset, state: name, dto);
    register!(registry, super::preset_commands::delete_openai_preset, state: dto);
    register!(registry, super::preset_commands::list_presets, state: api_id);
    register!(registry, super::preset_commands::preset_exists, state: name, api_id);
    register!(registry, super::preset_commands::get_preset, state: name, api_id);
    // provider_metadata_commands
    register!(registry, super::provider_metadata_commands::get_openrouter_model_providers, state: dto);
    register!(registry, super::provider_metadata_commands::get_openrouter_credits, state:);
    register!(registry, super::provider_metadata_commands::get_nanogpt_model_providers, state: dto);
    register!(registry, super::provider_metadata_commands::get_nanogpt_credits, state:);
    register!(registry, super::provider_metadata_commands::get_siliconflow_embedding_models, state: dto);
    register!(registry, super::provider_metadata_commands::get_workers_ai_embedding_models, state: dto);
    register!(registry, super::provider_metadata_commands::get_workers_ai_multimodal_models, state: dto);
    // quick_reply_commands
    register!(registry, super::quick_reply_commands::save_quick_reply_set, state: payload);
    register!(registry, super::quick_reply_commands::delete_quick_reply_set, state: payload);
    // resource_bridge_commands
    register_sync!(registry, super::resource_bridge_commands::read_frontend_template, state: name);
    register_sync!(registry, super::resource_bridge_commands::read_frontend_extension_template, state: extension, name);
    // runtime_paths_commands
    register_sync!(registry, super::runtime_paths_commands::get_runtime_paths, state:);
    register!(registry, super::runtime_paths_commands::set_data_root, state: data_root);
    // secret_commands
    register!(registry, super::secret_commands::write_secret, state: dto);
    register!(registry, super::secret_commands::read_secret_state, state:);
    register!(registry, super::secret_commands::read_secret_settings, state:);
    register!(registry, super::secret_commands::view_secrets, state:);
    register!(registry, super::secret_commands::find_secret, state: dto);
    register!(registry, super::secret_commands::delete_secret, state: dto);
    register!(registry, super::secret_commands::rotate_secret, state: dto);
    register!(registry, super::secret_commands::rename_secret, state: dto);
    // settings_commands
    register!(registry, super::settings_commands::get_rusttavern_settings, state:);
    register!(registry, super::settings_commands::get_chat_backup_storage_stats, state:);
    register!(registry, super::settings_commands::update_rusttavern_settings, state: dto);
    register!(registry, super::settings_commands::save_user_settings, state: settings);
    register!(registry, super::settings_commands::save_user_settings_patch, state: patch);
    register!(registry, super::settings_commands::get_sillytavern_settings, state:);
    register!(registry, super::settings_commands::create_settings_snapshot, state:);
    register!(registry, super::settings_commands::get_settings_snapshots, state:);
    register!(registry, super::settings_commands::load_settings_snapshot, state: name);
    register!(registry, super::settings_commands::restore_settings_snapshot, state: name);
    // skill_commands
    register!(registry, super::skill_commands::download_skill_import_url, state: url);
    register!(registry, super::skill_commands::list_skills, state: scope);
    register!(registry, super::skill_commands::list_skill_files, state: name, scope);
    register!(registry, super::skill_commands::preview_skill_import, state: input, target_scope);
    register!(registry, super::skill_commands::install_skill_import, state: request);
    register!(registry, super::skill_commands::read_skill_file, state: name, path, scope, max_chars, start_line, line_count, start_char);
    register!(registry, super::skill_commands::write_skill_file, state: name, path, content, scope, expected_sha256);
    register!(registry, super::skill_commands::export_skill, state: name, scope);
    register!(registry, super::skill_commands::delete_skill, state: name, scope);
    register!(registry, super::skill_commands::move_skill, state: request);
    register!(registry, super::skill_commands::retarget_skill_scope, state: request);
    // stable_diffusion_commands
    register!(registry, super::stable_diffusion_commands::sd_handle, state: request_id, path, body);
    register!(registry, super::stable_diffusion_commands::cancel_sd_request, state: request_id);
    // sync_automation_commands
    register!(registry, super::sync_automation_commands::sync_automation_get_config, state:);
    register!(registry, super::sync_automation_commands::sync_automation_update_config, state: config);
    register!(registry, super::sync_automation_commands::sync_automation_get_status, state:);
    // sync_commands
    register!(registry, super::sync_commands::sync_get_dataset_catalog, );
    // theme_commands
    register!(registry, super::theme_commands::save_theme, state: dto);
    register!(registry, super::theme_commands::delete_theme, state: dto);
    // tokenizer_commands
    register!(registry, super::tokenizer_commands::count_openai_tokens, state: dto);
    register!(registry, super::tokenizer_commands::count_openai_tokens_batch, state: dto);
    register!(registry, super::tokenizer_commands::count_openai_token_prefixes, state: dto);
    register!(registry, super::tokenizer_commands::encode_openai_tokens, state: dto);
    register!(registry, super::tokenizer_commands::decode_openai_tokens, state: dto);
    register!(registry, super::tokenizer_commands::build_openai_logit_bias, state: dto);
    // translate_commands
    register!(registry, super::translate_commands::translate_text, state: provider, body);
    // tts_commands
    register!(registry, super::tts_commands::tts_handle, state: path, body);
    // tt_sync_commands
    register!(registry, super::tt_sync_commands::tt_sync_pair, state: pair_uri);
    register!(registry, super::tt_sync_commands::tt_sync_list_servers, state:);
    register!(registry, super::tt_sync_commands::tt_sync_remove_server, state: server_device_id);
    register!(registry, super::tt_sync_commands::tt_sync_pull, state: server_device_id, mode, options);
    register!(registry, super::tt_sync_commands::tt_sync_push, state: server_device_id, mode, options);
    // update_commands
    register!(registry, super::update_commands::check_for_update, state: channel);
    // upload_staging_commands
    register!(registry, super::upload_staging_commands::stage_upload_begin, state: dto);
    register!(registry, super::upload_staging_commands::stage_upload_chunk, state: file_path, offset, data);
    register!(registry, super::upload_staging_commands::stage_upload_finish, state: file_path, expected_size);
    register!(registry, super::upload_staging_commands::stage_upload_discard, state: file_path);
    // user_commands
    register!(registry, super::user_commands::get_all_users, state:);
    register!(registry, super::user_commands::get_user, state: id);
    register!(registry, super::user_commands::get_user_by_username, state: username);
    register!(registry, super::user_commands::create_user, state: dto);
    register!(registry, super::user_commands::update_user, state: dto);
    register!(registry, super::user_commands::delete_user, state: id);
    // user_directory_commands
    register!(registry, super::user_directory_commands::get_user_directory, state: handle);
    register!(registry, super::user_directory_commands::ensure_user_directories_exist, state: handle);
    register!(registry, super::user_directory_commands::ensure_default_user_directories_exist, state:);
    // world_info_commands
    register!(registry, super::world_info_commands::get_world_info, state: dto);
    register!(registry, super::world_info_commands::get_world_infos_batch, state: dto);
    register!(registry, super::world_info_commands::normalize_world_info_name, state: dto);
    register!(registry, super::world_info_commands::save_world_info, state: dto);
    register!(registry, super::world_info_commands::delete_world_info, state: dto);
    register!(registry, super::world_info_commands::import_world_info, state: dto);

    // plugin compat (server mode): tauri plugin commands the frontend still
    // routes through the generic invoke dispatch. The registry names must
    // match the `plugin:*` command strings the frontend sends.
    register!(registry, super::plugin_compat_commands::plugin_fs_open, name: "plugin:fs|open", state: path, options);
    register!(registry, super::plugin_compat_commands::plugin_fs_read, name: "plugin:fs|read", state: rid, len);
    register!(registry, super::plugin_compat_commands::plugin_resources_close, name: "plugin:resources|close", state: rid);
    register!(registry, super::plugin_compat_commands::plugin_fs_remove, name: "plugin:fs|remove", state: path, options);
    register!(registry, super::plugin_compat_commands::plugin_fs_write_file, name: "plugin:fs|write_file", state: path, data);
    register!(registry, super::plugin_compat_commands::plugin_fs_mkdir, name: "plugin:fs|mkdir", state: path, options);
    register!(registry, super::plugin_compat_commands::plugin_dialog_open, name: "plugin:dialog|open");
    register!(registry, super::plugin_compat_commands::plugin_opener_open_url, name: "plugin:opener|open_url", url, with);
    register!(registry, super::plugin_compat_commands::plugin_opener_reveal_item_in_dir, name: "plugin:opener|reveal_item_in_dir", paths);

    registry
}