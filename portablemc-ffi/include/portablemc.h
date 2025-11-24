#ifndef _PORTABLEMC_H
#define _PORTABLEMC_H

#include <stdbool.h>
#include <stdint.h>
#include <stdbool.h>
#include <string.h>

#ifdef __cplusplus
extern "C" {
#endif

/// Microsoft Account authenticator.
typedef struct pmc_msa_auth pmc_msa_auth;

/// Microsoft Account device code flow authenticator.
typedef struct pmc_msa_device_code_flow pmc_msa_device_code_flow;

/// Microsoft Account device code flow authenticator.
typedef struct pmc_msa_account pmc_msa_account;

/// A file-backed database for storing accounts.
typedef struct pmc_msa_database pmc_msa_database;

/// The installer that supports the minimal standard format for version metadata with
/// support for libraries, assets and loggers automatic installation. By defaults, it 
/// also supports finding a suitable JVM for running the game.
typedef struct pmc_base pmc_base;

/// A structure representing an installed game.
typedef struct pmc_game pmc_game;

/// An installer for supporting Mojang-provided versions. It provides support for various
/// standard arguments such as demo mode, window resolution and quick play, it also 
/// provides various fixes for known issues of old versions.
typedef struct pmc_moj pmc_moj;

/// An installer for supporting mod loaders that are Fabric or like it (Quilt, 
/// LegacyFabric, Babric). The generic parameter is used to specify the API to use.
typedef struct pmc_fabric pmc_fabric;

/// An installer that supports Forge and NeoForge mod loaders.
typedef struct pmc_forge pmc_forge;

/// An array of 16 bytes representing an UUID.
typedef uint8_t pmc_uuid[16];

/// An array of 16 bytes representing an UUID.
typedef uint8_t pmc_sha1[20];

typedef struct {
    uint16_t width;
    uint16_t height;
} pmc_resolution;

/// The code of all errors.
typedef enum {
    // Uncategorized
    PMC_ERR_UNSET = 0x00,
    PMC_ERR_INTERNAL = 0x01,
    // MSA auth
    PMC_ERR_MSA_AUTH_DECLINED = 0x10,
    PMC_ERR_MSA_AUTH_TIMED_OUT,
    PMC_ERR_MSA_AUTH_OUTDATED_TOKEN,
    PMC_ERR_MSA_AUTH_DOES_NOT_OWN_GAME,
    PMC_ERR_MSA_AUTH_INVALID_STATUS,
    PMC_ERR_MSA_AUTH_UNKNOWN,
    // MSA database
    PMC_ERR_MSA_DATABASE_IO = 0x20,
    PMC_ERR_MSA_DATABASE_CORRUPTED,
    PMC_ERR_MSA_DATABASE_WRITE_FAILED,
    // Base installer
    PMC_ERR_BASE_HIERARCHY_LOOP = 0x30,
    PMC_ERR_BASE_VERSION_NOT_FOUND,
    PMC_ERR_BASE_ASSETS_NOT_FOUND,
    PMC_ERR_BASE_CLIENT_NOT_FOUND,
    PMC_ERR_BASE_LIBRARY_NOT_FOUND,
    PMC_ERR_BASE_JVM_NOT_FOUND,
    PMC_ERR_BASE_MAIN_CLASS_NOT_FOUND,
    PMC_ERR_BASE_DOWNLOAD_RESOURCES_CANCELLED,
    PMC_ERR_BASE_DOWNLOAD,
    // Mojang installer
    PMC_ERR_MOJ_LWJGL_FIX_NOT_FOUND = 0x40,
    // Fabric installer
    PMC_ERR_FABRIC_LATEST_VERSION_NOT_FOUND = 0x50,
    PMC_ERR_FABRIC_GAME_VERSION_NOT_FOUND,
    PMC_ERR_FABRIC_LOADER_VERSION_NOT_FOUND,
    // Forge installer
    PMC_ERR_FORGE_LATEST_VERSION_NOT_FOUND = 0x60,
    PMC_ERR_FORGE_INSTALLER_NOT_FOUND,
    PMC_ERR_FORGE_MAVEN_METADATA_MALFORMED,
    PMC_ERR_FORGE_INSTALLER_PROFILE_NOT_FOUND,
    PMC_ERR_FORGE_INSTALLER_PROFILE_INCOHERENT,
    PMC_ERR_FORGE_INSTALLER_VERSION_METADATA_NOT_FOUND,
    PMC_ERR_FORGE_INSTALLER_FILE_NOT_FOUND,
    PMC_ERR_FORGE_INSTALLER_PROCESSOR_NOT_FOUND,
    PMC_ERR_FORGE_INSTALLER_PROCESSOR_FAILED,
    PMC_ERR_FORGE_INSTALLER_PROCESSOR_CORRUPTED,
} pmc_err_tag;

/// PMC_ERR_INTERNAL
typedef struct {
    const char *origin;
} pmc_err_data_internal;

/// PMC_ERR_MSA_AUTH_INVALID_STATUS
typedef struct {
    uint16_t status;
} pmc_err_data_msa_auth_invalid_status;

/// PMC_ERR_MSA_AUTH_UNKNOWN
typedef struct {
    const char *message;
} pmc_err_data_msa_auth_unknown;

/// PMC_ERR_BASE_HIERARCHY_LOOP
typedef struct {
    const char *version;
} pmc_err_base_hierarchy_loop;

/// PMC_ERR_BASE_VERSION_NOT_FOUND
typedef struct {
    const char *version;
} pmc_err_base_version_not_found;

/// PMC_ERR_BASE_ASSETS_NOT_FOUND
typedef struct {
    const char *id;
} pmc_err_base_assets_not_found;

/// PMC_ERR_BASE_LIBRARY_NOT_FOUND
typedef struct {
    const char *name;
} pmc_err_base_library_not_found;

/// PMC_ERR_BASE_JVM_NOT_FOUND
typedef struct {
    uint32_t major_version;
} pmc_err_base_jvm_not_found;

/// PMC_ERR_MOJ_LWJGL_FIX_NOT_FOUND
typedef struct {
    const char *version;
} pmc_err_moj_lwjgl_fix_not_found;

/// PMC_ERR_FABRIC_LATEST_VERSION_NOT_FOUND
typedef struct {
    const char *game_version;  // Can be NULL
    bool stable;
} pmc_err_fabric_latest_version_not_found;

/// PMC_ERR_FABRIC_GAME_VERSION_NOT_FOUND
typedef struct {
    const char *game_version;
} pmc_err_fabric_game_version_not_found;

/// PMC_ERR_FABRIC_LOADER_VERSION_NOT_FOUND
typedef struct {
    const char *game_version;
    const char *loader_version;
} pmc_err_fabric_loader_version_not_found;

/// PMC_ERR_FORGE_LATEST_VERSION_NOT_FOUND
typedef struct {
    const char *game_version;
    bool stable;
} pmc_err_forge_latest_version_not_found;

/// PMC_ERR_FORGE_INSTALLER_NOT_FOUND
typedef struct {
    const char *version;
} pmc_err_forge_installer_not_found;

/// PMC_ERR_FORGE_INSTALLER_FILE_NOT_FOUND
typedef struct {
    const char *entry;
} pmc_err_forge_installer_file_not_found;

/// PMC_ERR_FORGE_INSTALLER_PROCESSOR_NOT_FOUND
typedef struct {
    const char *name;
} pmc_err_forge_installer_processor_not_found;

/// PMC_ERR_FORGE_INSTALLER_PROCESSOR_FAILED
typedef struct {
    const char *name;
    int status;
    size_t stdout_len;
    const char *stdout;
    size_t stderr_len;
    const char *stderr;
} pmc_err_forge_installer_processor_failed;

/// PMC_ERR_FORGE_INSTALLER_PROCESSOR_CORRUPTED
typedef struct {
    const char *name;
    const char *file;
    const pmc_sha1 *expected_sha1;
} pmc_err_forge_installer_processor_corrupted;

/// The union of all data types for errors.
typedef union {
    int _none;  // Ensure alignment for tag
    pmc_err_data_internal internal;
    pmc_err_data_msa_auth_invalid_status msa_auth_invalid_status;
    pmc_err_data_msa_auth_unknown msa_auth_unknown;
    pmc_err_base_hierarchy_loop base_hierarchy_loop;
    pmc_err_base_version_not_found base_version_not_found;
    pmc_err_base_assets_not_found base_assets_not_found;
    pmc_err_base_library_not_found base_library_not_found;
    pmc_err_base_jvm_not_found base_jvm_not_found;
    // TODO: download
    pmc_err_moj_lwjgl_fix_not_found moj_lwjgl_fix_not_found;
    pmc_err_fabric_latest_version_not_found fabric_latest_version_not_found;
    pmc_err_fabric_game_version_not_found fabric_game_version_not_found;
    pmc_err_fabric_loader_version_not_found fabric_loader_version_not_found;
    pmc_err_forge_latest_version_not_found forge_latest_version_not_found;
    pmc_err_forge_installer_not_found forge_installer_not_found;
    pmc_err_forge_installer_file_not_found forge_installer_file_not_found;
    pmc_err_forge_installer_processor_not_found forge_installer_processor_not_found;
    pmc_err_forge_installer_processor_failed forge_installer_processor_failed;
    pmc_err_forge_installer_processor_corrupted forge_installer_processor_corrupted;
} pmc_err_data;

/// Generic error type, you should usually use this type by defining a null-pointer to it 
/// and passing a pointer to that pointer to any function that accepts it. If an error
/// happens, the function will allocate an error and then write its pointer in the given
/// location. The error should be freed afterward.
///
/// This structure has a known layout in C.
typedef struct {
    /// Tag of the error.
    pmc_err_tag tag;
    /// The data of the tag, that can be used depending on the error tag.
    pmc_err_data data;
    /// The descriptive human-readable message for the error.
    const char *message;
} pmc_err;

/// The tag for the pmc_jvm_policy tagged union.
typedef enum {
    PMC_JVM_POLICY_STATIC,
    PMC_JVM_POLICY_SYSTEM,
    PMC_JVM_POLICY_MOJANG,
    PMC_JVM_POLICY_SYSTEM_THEN_MOJANG,
    PMC_JVM_POLICY_MOJANG_THEN_SYSTEM,
} pmc_jvm_policy_tag;

/// The JVM policy tagged union, only the static policy requires an explicit path value.
typedef struct {
    pmc_jvm_policy_tag tag;
    const char *static_path;
} pmc_jvm_policy;

/// Represent the release channel for a version.
typedef enum {
    PMC_VERSION_CHANNEL_UNSPECIFIED,
    PMC_VERSION_CHANNEL_RELEASE,
    PMC_VERSION_CHANNEL_SNAPSHOT,
    PMC_VERSION_CHANNEL_BETA,
    PMC_VERSION_CHANNEL_ALPHA,
} pmc_version_channel;

/// Represent a version loaded during the installation.
typedef struct {
    const char *name;
    const char *dir;
    pmc_version_channel channel;
} pmc_loaded_version;

/// Represent a version loaded during the installation.
typedef struct {
    const char *url;      // Not NULL
    uint32_t size;        // MAX to disable
    const pmc_sha1 *sha1; // NULL to disable
} pmc_library_download;

/// Represent a version loaded during the installation.
typedef struct {
    const char *gav;
    const char *path;
    const pmc_library_download *download;
    bool natives;
} pmc_loaded_library;

typedef struct {
    size_t len;
    const char *const *args;
} pmc_game_args;

/// The code of all events.
typedef enum {
    // Base installer
    PMC_EVENT_BASE_FILTER_FEATURES = 0x0,
    PMC_EVENT_BASE_LOADED_FEATURES,
    PMC_EVENT_BASE_LOAD_HIERARCHY,
    PMC_EVENT_BASE_LOADED_HIERARCHY,
    PMC_EVENT_BASE_LOAD_VERSION,
    PMC_EVENT_BASE_NEED_VERSION,
    PMC_EVENT_BASE_LOADED_VERSION,
    PMC_EVENT_BASE_LOAD_CLIENT,
    PMC_EVENT_BASE_LOADED_CLIENT,
    PMC_EVENT_BASE_LOAD_LIBRARIES,
    PMC_EVENT_BASE_FILTER_LIBRARIES,
    PMC_EVENT_BASE_LOADED_LIBRARIES,
    PMC_EVENT_BASE_FILTER_LIBRARIES_FILES,
    PMC_EVENT_BASE_LOADED_LIBRARIES_FILES,
    PMC_EVENT_BASE_NO_LOGGER,
    PMC_EVENT_BASE_LOAD_LOGGER,
    PMC_EVENT_BASE_LOADED_LOGGER,
    PMC_EVENT_BASE_NO_ASSETS,
    PMC_EVENT_BASE_LOAD_ASSETS,
    PMC_EVENT_BASE_LOADED_ASSETS,
    PMC_EVENT_BASE_VERIFIED_ASSETS,
    PMC_EVENT_BASE_LOAD_JVM,
    PMC_EVENT_BASE_FOUND_JVM_VERSION,
    PMC_EVENT_BASE_WARN_JVM_UNSUPPORTED_DYNAMIC_CTR,
    PMC_EVENT_BASE_WARN_JVM_UNSUPPORTED_PLATFORM,
    PMC_EVENT_BASE_WARN_JVM_MISSING_DISTRIBUTION,
    PMC_EVENT_BASE_LOADED_JVM,
    PMC_EVENT_BASE_DOWNLOAD_RESOURCES,
    PMC_EVENT_BASE_DOWNLOAD_PROGRESS,
    PMC_EVENT_BASE_DOWNLOADED_RESOURCES,
    PMC_EVENT_BASE_EXTRACTED_BINARIES,
    // Mojang installer
    PMC_EVENT_MOJ_INVALIDATED_VERSION = 0x50,
    PMC_EVENT_MOJ_FETCH_VERSION,
    PMC_EVENT_MOJ_FETCHED_VERSION,
    PMC_EVENT_MOJ_FIXED_LEGACY_QUICK_PLAY,
    PMC_EVENT_MOJ_FIXED_LEGACY_PROXY,
    PMC_EVENT_MOJ_FIXED_LEGACY_MERGE_SORT,
    PMC_EVENT_MOJ_FIXED_LEGACY_RESOLUTION,
    PMC_EVENT_MOJ_FIXED_BROKEN_AUTHLIB,
    PMC_EVENT_MOJ_WARN_UNSUPPORTED_QUICK_PLAY,
    PMC_EVENT_MOJ_WARN_UNSUPPORTED_RESOLUTION,
    // Fabric installer
    PMC_EVENT_FABRIC_FETCH_VERSION = 0x60,
    PMC_EVENT_FABRIC_FETCHED_VERSION,
    // Forge installer
    PMC_EVENT_FORGE_INSTALLING = 0x70,
    PMC_EVENT_FORGE_FETCH_INSTALLER,
    PMC_EVENT_FORGE_FETCHED_INSTALLER,
    PMC_EVENT_FORGE_INSTALLING_GAME,
    PMC_EVENT_FORGE_FETCH_INSTALLER_LIBRARIES,
    PMC_EVENT_FORGE_FETCHED_INSTALLER_LIBRARIES,
    PMC_EVENT_FORGE_RUN_INSTALLER_PROCESSOR,
    PMC_EVENT_FORGE_INSTALLED,
} pmc_event_tag;

/// PMC_EVENT_BASE_LOADED_FEATURES
typedef struct {
    size_t features_len;
    const char *const *features;
} pmc_event_base_loaded_features;

/// PMC_EVENT_BASE_LOAD_HIERARCHY
typedef struct {
    const char *root_version;
} pmc_event_base_load_hierarchy;

/// PMC_EVENT_BASE_LOADED_HIERARCHY
typedef struct {
    size_t hierarchy_len;
    const pmc_loaded_version *hierarchy;
} pmc_event_base_loaded_hierarchy;

/// PMC_EVENT_BASE_LOAD_VERSION 
typedef struct {
    const char *version;
    const char *file;
} pmc_event_base_load_version;

/// PMC_EVENT_BASE_LOADED_VERSION 
typedef struct {
    const char *version;
    const char *file;
} pmc_event_base_loaded_version;

/// PMC_EVENT_BASE_NEED_VERSION
typedef struct {
    const char *version;
    const char *file;
    bool *retry;
} pmc_event_base_need_version;

/// PMC_EVENT_BASE_LOADED_VERSION
typedef struct {
    const char *file;
} pmc_event_base_loaded_client;

/// PMC_EVENT_BASE_LOADED_LIBRARIES
typedef struct {
    size_t libraries_len;
    const pmc_loaded_library *libraries;
} pmc_event_base_loaded_libraries;

/// PMC_EVENT_BASE_LOADED_LIBRARIES_FILES
typedef struct {
    size_t class_files_len;
    const char *const *class_files;
    size_t natives_files_len;
    const char *const *natives_files;
} pmc_event_base_loaded_libraries_files;

/// PMC_EVENT_BASE_LOAD_LOGGER
typedef struct {
    const char *id;
} pmc_event_base_load_logger;

/// PMC_EVENT_BASE_LOADED_LOGGER
typedef struct {
    const char *id;
} pmc_event_base_loaded_logger;

/// PMC_EVENT_BASE_LOAD_ASSETS
typedef struct {
    const char *id;
} pmc_event_base_load_assets;

/// PMC_EVENT_BASE_LOADED_ASSETS
typedef struct {
    const char *id;
    size_t count;
} pmc_event_base_loaded_assets;

/// PMC_EVENT_BASE_VERIFIED_ASSETS
typedef struct {
    const char *id;
    size_t count;
} pmc_event_base_verified_assets;

/// PMC_EVENT_BASE_LOAD_JVM
typedef struct {
    uint32_t major_version;
} pmc_event_base_load_jvm;

/// PMC_EVENT_BASE_FOUND_JVM_VERSION
typedef struct {
    const char *file;
    const char *version;
    bool compatible;
} pmc_event_base_found_jvm_system_version;

/// PMC_EVENT_BASE_LOADED_JVM
typedef struct {
    const char *file;
    const char *version;  // Can be NULL if unknown ver.
    bool compatible;
} pmc_event_base_loaded_jvm;

/// PMC_EVENT_BASE_DOWNLOAD_RESOURCES
typedef struct {
    bool *cancel;  // Default to false, change to true to abort installation.
} pmc_event_base_download_resources;

/// PMC_EVENT_BASE_DOWNLOAD_PROGRESS
typedef struct {
    uint32_t count;
    uint32_t total_count;
    uint32_t size;
    uint32_t total_size;
} pmc_event_base_download_progress;

/// PMC_EVENT_BASE_EXTRACTED_BINARIES
typedef struct {
    const char *dir;
} pmc_event_base_extracted_binaries;

/// PMC_EVENT_MOJ_INVALIDATED_VERSION
typedef struct {
    const char *version;
} pmc_event_moj_invalidated_version;

/// PMC_EVENT_MOJ_FETCH_VERSION
typedef struct {
    const char *version;
} pmc_event_moj_fetch_version;

/// PMC_EVENT_MOJ_FETCHED_VERSION
typedef struct {
    const char *version;
} pmc_event_moj_fetched_version;

/// PMC_EVENT_MOJ_FIXED_LEGACY_PROXY
typedef struct {
    const char *host;
    uint16_t port;
} pmc_event_moj_fixed_legacy_proxy;

/// PMC_EVENT_FABRIC_FETCH_VERSION
typedef struct {
    const char *game_version;
    const char *loader_version;
} pmc_event_fabric_fetch_version;

/// PMC_EVENT_FABRIC_FETCHED_VERSION
typedef struct {
    const char *game_version;
    const char *loader_version;
} pmc_event_fabric_fetched_version;

/// PMC_EVENT_FORGE_INSTALLING
typedef struct {
    const char *tmp_dir;
} pmc_event_forge_installing;

/// PMC_EVENT_FORGE_FETCH_INSTALLER
typedef struct {
    const char *version;
} pmc_event_forge_fetch_installer;

/// PMC_EVENT_FORGE_FETCHED_INSTALLER
typedef struct {
    const char *version;
} pmc_event_forge_fetched_installer;

/// PMC_EVENT_FORGE_RUN_INSTALLER_PROCESSOR
typedef struct {
    const char *name;
    const char *task;
} pmc_event_forge_run_installer_processor;

/// The data accompanying an event, when relevant.
typedef union {
    int _none;
    pmc_event_base_loaded_features base_loaded_features;
    pmc_event_base_load_hierarchy base_load_hierarchy;
    pmc_event_base_loaded_hierarchy base_loaded_hierarchy;
    pmc_event_base_load_version base_load_version;
    pmc_event_base_loaded_version base_loaded_version;
    pmc_event_base_need_version base_need_version;
    pmc_event_base_loaded_client base_loaded_client;
    pmc_event_base_loaded_libraries base_loaded_libraries;
    pmc_event_base_loaded_libraries_files base_loaded_libraries_files;
    pmc_event_base_load_logger base_load_logger;
    pmc_event_base_loaded_logger base_loaded_logger;
    pmc_event_base_load_assets base_load_assets;
    pmc_event_base_loaded_assets base_loaded_assets;
    pmc_event_base_verified_assets base_verified_assets;
    pmc_event_base_load_jvm base_load_jvm;
    pmc_event_base_found_jvm_system_version base_found_jvm_system_version;
    pmc_event_base_loaded_jvm base_loaded_jvm;
    pmc_event_base_download_resources base_download_resources;
    pmc_event_base_download_progress base_download_progress;
    pmc_event_base_extracted_binaries base_extracted_binaries;
    pmc_event_moj_invalidated_version moj_invalidated_version;
    pmc_event_moj_fetch_version moj_fetch_version;
    pmc_event_moj_fetched_version moj_fetched_version;
    pmc_event_moj_fixed_legacy_proxy moj_fixed_legacy_proxy;
    pmc_event_fabric_fetch_version fabric_fetch_version;
    pmc_event_fabric_fetched_version fabric_fetched_version;
    pmc_event_forge_installing forge_installing;
    pmc_event_forge_fetch_installer forge_fetch_installer;
    pmc_event_forge_fetched_installer forge_fetched_installer;
    pmc_event_forge_run_installer_processor forge_run_installer_processor;
} pmc_event_data;

/// The full event structure.
typedef struct {
    pmc_event_tag tag;
    pmc_event_data data;
} pmc_event;

/// A generic event handler.
typedef void (*pmc_handler)(pmc_event *event);


/// A generic function to free any pointer that has been returned by a PortableMC 
/// function, unless const or if explicitly stated.
void pmc_free(void *ptr);

/// Create a new authenticator with the given Azure application id (client id).
pmc_msa_auth *pmc_msa_auth_new(const char *app_id);
/// Return the Azure application id (client id) configured for that auth object.
char *pmc_msa_auth_app_id(const pmc_msa_auth *auth);
char *pmc_msa_auth_language_code(const pmc_msa_auth *auth);
void  pmc_msa_auth_set_language_code(pmc_msa_auth *auth, const char *code);
pmc_msa_device_code_flow *pmc_msa_auth_request_device_code(const pmc_msa_auth *auth, pmc_err **err);

char *pmc_msa_device_code_flow_app_id(const pmc_msa_device_code_flow *flow);
char *pmc_msa_device_code_flow_user_code(const pmc_msa_device_code_flow *flow);
char *pmc_msa_device_code_flow_verification_uri(const pmc_msa_device_code_flow *flow);
char *pmc_msa_device_code_flow_message(const pmc_msa_device_code_flow *flow);
pmc_msa_account *pmc_msa_device_code_flow_wait(const pmc_msa_device_code_flow *flow, pmc_err **err);

char *pmc_msa_account_app_id(const pmc_msa_account *acc);
char *pmc_msa_account_access_token(const pmc_msa_account *acc);
pmc_uuid *pmc_msa_account_uuid(const pmc_msa_account *acc);
char *pmc_msa_account_username(const pmc_msa_account *acc);
char *pmc_msa_account_xuid(const pmc_msa_account *acc);
void  pmc_msa_account_request_profile(pmc_msa_account *acc, pmc_err **err);
void  pmc_msa_account_request_refresh(pmc_msa_account *acc, pmc_err **err);

pmc_msa_database *pmc_msa_database_new(const char *file);
char *pmc_msa_database_file(const pmc_msa_database *database);
pmc_msa_account *pmc_msa_database_load_from_uuid(const pmc_msa_database *database, const pmc_uuid *uuid, pmc_err **err);
pmc_msa_account *pmc_msa_database_load_from_username(const pmc_msa_database *database, const char *username, pmc_err **err);
pmc_msa_account *pmc_msa_database_remove_from_uuid(const pmc_msa_database *database, const pmc_uuid *uuid, pmc_err **err);
pmc_msa_account *pmc_msa_database_remove_from_username(const pmc_msa_database *database, const char *username, pmc_err **err);
void pmc_msa_database_store(const pmc_msa_database *database, pmc_msa_account *acc, pmc_err **err);

pmc_base *pmc_base_new(const char *version);
char *pmc_base_version(const pmc_base *inst);
void  pmc_base_set_version(pmc_base *inst, const char *version);
char *pmc_base_versions_dir(const pmc_base *inst);
void  pmc_base_set_versions_dir(pmc_base *inst, const char *dir);
char *pmc_base_libraries_dir(const pmc_base *inst);
void  pmc_base_set_libraries_dir(pmc_base *inst, const char *dir);
char *pmc_base_assets_dir(const pmc_base *inst);
void  pmc_base_set_assets_dir(pmc_base *inst, const char *dir);
char *pmc_base_jvm_dir(const pmc_base *inst);
void  pmc_base_set_jvm_dir(pmc_base *inst, const char *dir);
char *pmc_base_bin_dir(const pmc_base *inst);
void  pmc_base_set_bin_dir(pmc_base *inst, const char *dir);
char *pmc_base_mc_dir(const pmc_base *inst);
void  pmc_base_set_mc_dir(pmc_base *inst, const char *dir);
void  pmc_base_set_main_dir(pmc_base *inst, const char *dir);
bool  pmc_base_strict_assets_check(const pmc_base *inst);
void  pmc_base_set_strict_assets_check(pmc_base *inst, bool strict);
bool  pmc_base_strict_libraries_check(const pmc_base *inst);
void  pmc_base_set_strict_libraries_check(pmc_base *inst, bool strict);
bool  pmc_base_strict_jvm_check(const pmc_base *inst);
void  pmc_base_set_strict_jvm_check(pmc_base *inst, bool strict);
pmc_jvm_policy *pmc_base_jvm_policy(const pmc_base *inst);
void  pmc_base_set_jvm_policy(pmc_base *inst, pmc_jvm_policy policy);
char *pmc_base_launcher_name(const pmc_base *inst);
void  pmc_base_set_launcher_name(pmc_base *inst, const char *name);
char *pmc_base_launcher_version(const pmc_base *inst);
void  pmc_base_set_launcher_version(pmc_base *inst, const char *version);
pmc_game *pmc_base_install(pmc_base *inst, pmc_handler handler, pmc_err **err);

char *pmc_game_jvm_file(const pmc_game *game);
char *pmc_game_mc_dir(const pmc_game *game);
char *pmc_game_main_class(const pmc_game *game);
pmc_game_args *pmc_game_jvm_args(const pmc_game *game);
pmc_game_args *pmc_game_game_args(const pmc_game *game);
uint32_t pmc_game_spawn(const pmc_game *game, pmc_err **err);

extern const char *pmc_moj_release;
extern const char *pmc_moj_snapshot;
pmc_moj *pmc_moj_new(const char *version);
const pmc_game *pmc_moj_base(const pmc_moj *inst);
pmc_game *pmc_moj_base_mut(pmc_moj *inst)
const char *pmc_moj_version(const pmc_moj *inst);
void pmc_moj_set_version(pmc_moj *inst, const char *version);
// TODO: fetch excludes
bool pmc_moj_demo(const pmc_moj *inst);
void pmc_moj_set_demo(pmc_moj *inst);
// pmc_moj_quick_play(const pmc_moj *inst);
pmc_resolution pmc_moj_resolution(const pmc_moj *inst);
void pmc_moj_set_resolution(pmc_moj *inst, uint16_t width, uint16_t height);
void pmc_moj_remove_resolution(pmc_moj *inst);
bool pmc_moj_disable_multiplayer(const pmc_moj *inst);

#ifdef __cplusplus
}
#endif

#endif // _PORTABLEMC_H
