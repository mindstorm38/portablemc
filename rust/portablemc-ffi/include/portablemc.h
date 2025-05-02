#ifndef _PORTABLEMC_H
#define _PORTABLEMC_H

#include <stdbool.h>
#include <stdint.h>
#include <stdbool.h>
#include <string.h>

#ifdef __cplusplus
extern "C" {
#endif

/// Generic
#define PMC_ERR_INTERNAL                        0x01
// MSA Auth
#define PMC_ERR_MSA_AUTH_DECLINED               0x10
#define PMC_ERR_MSA_AUTH_TIMED_OUT              0x11
#define PMC_ERR_MSA_AUTH_OUTDATED_TOKEN         0x12
#define PMC_ERR_MSA_AUTH_DOES_NOT_OWN_GAME      0x13
#define PMC_ERR_MSA_AUTH_INVALID_STATUS         0x14
#define PMC_ERR_MSA_AUTH_UNKNOWN                0x15
// MSA Database
#define PMC_ERR_MSA_DATABASE_IO                 0x20
#define PMC_ERR_MSA_DATABASE_CORRUPTED          0x21
#define PMC_ERR_MSA_DATABASE_WRITE_FAILED       0x22
// Standard game
#define PMC_ERR_STANDARD_HIERARCHY_LOOP         0x30
#define PMC_ERR_STANDARD_VERSION_NOT_FOUND      0x31
#define PMC_ERR_STANDARD_ASSETS_NOT_FOUND       0x32
#define PMC_ERR_STANDARD_CLIENT_NOT_FOUND       0x33
#define PMC_ERR_STANDARD_LIBRARY_NOT_FOUND      0x34
#define PMC_ERR_STANDARD_JVM_NOT_FOUND          0x35
#define PMC_ERR_STANDARD_MAIN_CLASS_NOT_FOUND   0x36
#define PMC_ERR_STANDARD_DOWNLOAD_RESOURCES_CANCELLED 0x37
#define PMC_ERR_STANDARD_DOWNLOAD               0x38

/// An array of 16 bytes representing an UUID.
typedef uint8_t pmc_uuid[16];

/// An variable-length array, called vector.
typedef struct pmc_vec pmc_vec;

/// Generic error type, you should usually use this type by defining a null-pointer to it 
/// and passing a pointer to that pointer to any function that accepts it. If an error
/// happens, the function will allocate an error and then write its pointer in the given
/// location. The error should be freed afterward.
///
/// This structure has a known layout in C, but more information can be accessed using 
/// the `pmc_err_` function, to get the message or the associated data.
typedef struct pmc_err pmc_err;

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
typedef struct pmc_standard pmc_standard;

/// The tag for the pmc_jvm_policy tagged union.
typedef enum pmc_jvm_policy_tag {
    PMC_JVM_POLICY_STATIC,
    PMC_JVM_POLICY_SYSTEM,
    PMC_JVM_POLICY_MOJANG,
    PMC_JVM_POLICY_SYSTEM_THEN_MOJANG,
    PMC_JVM_POLICY_MOJANG_THEN_SYSTEM,
} pmc_jvm_policy_tag;

/// The JVM policy tagged union, only the static policy requires an explicit path value.
typedef struct pmc_jvm_policy {
    pmc_jvm_policy_tag tag;
    const char *static_path;
} pmc_jvm_policy;

typedef enum pmc_version_channel {
    PMC_VERSION_CHANNEL_RELEASE,
    PMC_VERSION_CHANNEL_SNAPSHOT,
    PMC_VERSION_CHANNEL_BETA,
    PMC_VERSION_CHANNEL_ALPHA,
} pmc_version_channel;

typedef struct pmc_loaded_version {
    const char *name;
    const char *dir;
    pmc_version_channel channel;
} pmc_loaded_version;

typedef enum pmc_standard_event_tag {
    PMC_EVENT_FILTER_FEATURES,
    PMC_EVENT_LOADED_FEATURES,
    PMC_EVENT_LOAD_HIERARCHY,
    PMC_EVENT_LOADED_HIERARCHY,
    PMC_EVENT_LOAD_VERSION,
    PMC_EVENT_NEED_VERSION,
    PMC_EVENT_LOADED_VERSION,
    PMC_EVENT_LOAD_CLIENT,
    PMC_EVENT_LOADED_CLIENT,
    PMC_EVENT_LOAD_LIBRARIES,
    PMC_EVENT_FILTER_LIBRARIES,
    PMC_EVENT_LOADED_LIBRARIES,
    PMC_EVENT_FILTER_LIBRARIES_FILES,
    PMC_EVENT_LOADED_LIBRARIES_FILES,
} pmc_standard_event_tag;

typedef union pmc_standard_event {

} pmc_standard_event;

typedef struct pmc_standard_handler {
    // Features
    void (*filter_features)();
    void (*loaded_features)();
    // Hierarchy
    void (*load_hierarchy)(const char *root_version);
    void (*loaded_hierarchy)();
    // Individual versions
    void (*load_version)(const char *version, const char *file);
    bool (*need_version)(const char *version, const char *file);
    void (*loaded_version)(const char *version, const char *file);
    // Client
    void (*load_client)();
    void (*loaded_client)(const char *file);
    // Libraries
    void (*load_libraries)();
    void (*filter_libraries)();
    void (*loaded_libraries)();
    void (*filter_libraries_files)();
    void (*loaded_libraries_files)();
    // Logger
    void (*no_logger)();
    void (*load_logger)(const char *id);
    void (*loaded_logger)(const char *id);
    // Assets
    void (*no_assets)();
    void (*load_assets)(const char *id);
    void (*loaded_assets)(const char *id, size_t count);
    void (*verified_assets)(const char *id, size_t count);
    // JVM
    void (*load_jvm)(uint32_t major_version);
    void (*found_jvm_system_version)(const char *file, const char *version, bool compatible);
    void (*warn_jvm_unsupported_dynamic_crt)();
    void (*warn_jvm_unsupported_platform)();
    void (*warn_jvm_missing_distribution)();
    void (*loaded_jvm)(const char *file, const char *version, bool compatible);
} pmc_standard_handler;

/// A generic function to free any pointer that has been returned by a PortableMC 
/// function, unless const or if explicitly stated.
void pmc_free(void *ptr);

/// Get the element at the given place in the vector. The returned pointer must be casted
/// to the type expected from that vector.
void *pmc_vec_get(size_t index);
/// Get a number of elements from the vector.
void *pmc_vec_get_many(size_t index, size_t count);

/// Retrieve the code of the given error.
uint8_t pmc_err_code(const pmc_err *err);
/// Return any data associated to the error code, interpretation of that data pointer 
/// depends on the error code and should be freed with `pmc_free`.
void *pmc_err_data(const pmc_err *err);
/// Retrieve the message description of the given error, should be freed.
char *pmc_err_message(const pmc_err *err);

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

pmc_standard *pmc_standard_new(const char *version);
char *pmc_standard_version(const pmc_standard *inst);
void  pmc_standard_set_version(pmc_standard *inst, const char *version);
char *pmc_standard_versions_dir(const pmc_standard *inst);
void  pmc_standard_set_versions_dir(pmc_standard *inst, const char *dir);
char *pmc_standard_libraries_dir(const pmc_standard *inst);
void  pmc_standard_set_libraries_dir(pmc_standard *inst, const char *dir);
char *pmc_standard_assets_dir(const pmc_standard *inst);
void  pmc_standard_set_assets_dir(pmc_standard *inst, const char *dir);
char *pmc_standard_jvm_dir(const pmc_standard *inst);
void  pmc_standard_set_jvm_dir(pmc_standard *inst, const char *dir);
char *pmc_standard_bin_dir(const pmc_standard *inst);
void  pmc_standard_set_bin_dir(pmc_standard *inst, const char *dir);
char *pmc_standard_mc_dir(const pmc_standard *inst);
void  pmc_standard_set_mc_dir(pmc_standard *inst, const char *dir);
void  pmc_standard_set_main_dir(pmc_standard *inst, const char *dir);
bool  pmc_standard_strict_assets_check(const pmc_standard *inst);
void  pmc_standard_set_strict_assets_check(pmc_standard *inst, bool strict);
bool  pmc_standard_strict_libraries_check(const pmc_standard *inst);
void  pmc_standard_set_strict_libraries_check(pmc_standard *inst, bool strict);
bool  pmc_standard_strict_jvm_check(const pmc_standard *inst);
void  pmc_standard_set_strict_jvm_check(pmc_standard *inst, bool strict);
pmc_jvm_policy *pmc_standard_jvm_policy(const pmc_standard *inst);
void  pmc_standard_set_jvm_policy(pmc_standard *inst, pmc_jvm_policy policy);
char *pmc_standard_launcher_name(const pmc_standard *inst);
void  pmc_standard_set_launcher_name(pmc_standard *inst, const char *name);
char *pmc_standard_launcher_version(const pmc_standard *inst);
void  pmc_standard_set_launcher_version(pmc_standard *inst, const char *version);
pmc_game *pmc_standard_install(pmc_standard *inst, const pmc_standard_handler *handler, pmc_err **err);

#ifdef __cplusplus
}
#endif

#endif // _PORTABLEMC_H