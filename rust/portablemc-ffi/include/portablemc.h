#ifndef _PORTABLEMC_H
#define _PORTABLEMC_H

#include <stdbool.h>
#include <stdint.h>
#include <stdbool.h>
#include <string.h>

#ifdef __cplusplus
extern "C" {
#endif

/// An array of 16 bytes representing an UUID.
typedef uint8_t pmc_uuid[16];

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
    // Standard installer
    PMC_ERR_BASE_HIERARCHY_LOOP = 0x30,
    PMC_ERR_BASE_VERSION_NOT_FOUND,
    PMC_ERR_BASE_ASSETS_NOT_FOUND,
    PMC_ERR_BASE_CLIENT_NOT_FOUND,
    PMC_ERR_BASE_LIBRARY_NOT_FOUND,
    PMC_ERR_BASE_JVM_NOT_FOUND,
    PMC_ERR_BASE_MAIN_CLASS_NOT_FOUND,
    PMC_ERR_BASE_DOWNLOAD_RESOURCES_CANCELLED,
    PMC_ERR_BASE_DOWNLOAD
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

/// PMC_ERR_BASE_JVM_NOT_FOUND
typedef struct {
    uint32_t major_version;
} pmc_err_base_jvm_not_found;

/// The union of all data types for errors.
typedef union {
    int _none;  // Ensure alignment for tag
    pmc_err_data_internal internal;
    pmc_err_data_msa_auth_invalid_status msa_auth_invalid_status;
    pmc_err_data_msa_auth_unknown msa_auth_unknown;
    pmc_err_base_hierarchy_loop std_hierarchy_loop;
    pmc_err_base_version_not_found std_version_not_found;
    pmc_err_base_assets_not_found std_assets_not_found;
    pmc_err_base_jvm_not_found std_jvm_not_found;
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

/// Microsoft Account authenticator.
typedef struct pmc_msa_auth pmc_msa_auth;

/// Microsoft Account device code flow authenticator.
typedef struct pmc_msa_device_code_flow pmc_msa_device_code_flow;

/// Microsoft Account device code flow authenticator.
typedef struct pmc_msa_account pmc_msa_account;

/// A file-backed database for storing accounts.
typedef struct pmc_msa_database pmc_msa_database;

/// A structure representing an installed game.
typedef struct pmc_game pmc_game;

/// The installer that supports the minimal standard format for version metadata with
/// support for libraries, assets and loggers automatic installation. By defaults, it 
/// also supports finding a suitable JVM for running the game.
typedef struct pmc_base pmc_base;

/// An installer for supporting Mojang-provided versions. It provides support for various
/// standard arguments such as demo mode, window resolution and quick play, it also 
/// provides various fixes for known issues of old versions.
typedef struct pmc_mojang pmc_mojang;

/// An installer for supporting mod loaders that are Fabric or like it (Quilt, 
/// LegacyFabric, Babric). The generic parameter is used to specify the API to use.
typedef struct pmc_fabric pmc_fabric;

/// An installer that supports Forge and NeoForge mod loaders.
typedef struct pmc_forge pmc_forge;

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

typedef enum {
    PMC_VERSION_CHANNEL_RELEASE,
    PMC_VERSION_CHANNEL_SNAPSHOT,
    PMC_VERSION_CHANNEL_BETA,
    PMC_VERSION_CHANNEL_ALPHA,
} pmc_version_channel;

typedef struct {
    const char *name;
    const char *dir;
    pmc_version_channel channel;
} pmc_loaded_version;

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
    PMC_EVT__ = 0x50,
} pmc_event_tag;

/// PMC_EVENT_BASE_LOAD_HIERARCHY
typedef struct {
    const char *root_version;
} pmc_event_base_load_hierarchy;

/// PMC_EVENT_BASE_LOADED_HIERARCHY
typedef struct {
    const pmc_loaded_version *hierarchy;
    size_t hierarchy_len;
} pmc_event_base_loaded_hierarchy;

typedef union {
    int _none;
    pmc_event_base_load_hierarchy base_load_hierarchy;
    pmc_event_base_loaded_hierarchy base_loaded_hierarchy;
} pmc_event_data;

typedef struct {
    pmc_event_tag tag;
    pmc_event_data data;
} pmc_event;

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

char *pmc_game_jvm_file(const pmc_game *game);
char *pmc_game_mc_dir(const pmc_game *game);
char *pmc_game_main_class(const pmc_game *game);
char *pmc_game_jvm_args(const pmc_game *game);
char *pmc_game_game_args(const pmc_game *game);

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
// pmc_game *pmc_base_install(pmc_base *inst, const pmc_standard_handler *handler, pmc_err **err);

#ifdef __cplusplus
}
#endif

#endif // _PORTABLEMC_H