#ifndef _PORTABLEMC_H
#define _PORTABLEMC_H

#include <stdbool.h>
#include <stdint.h>
#include <stdbool.h>
#include <string.h>

#ifdef __cplusplus
extern "C" {
#endif

// Generic
#define PMC_ERR_INTERNAL                        0x01
// MSA Auth
#define PMC_ERR_MSA_AUTH_DECLINED               0x40
#define PMC_ERR_MSA_AUTH_TIMED_OUT              0x41
#define PMC_ERR_MSA_AUTH_OUTDATED_TOKEN         0x42
#define PMC_ERR_MSA_AUTH_DOES_NOT_OWN_GAME      0x43
#define PMC_ERR_MSA_AUTH_INVALID_STATUS         0x44
#define PMC_ERR_MSA_AUTH_UNKNOWN                0x45
// MSA Database
#define PMC_ERR_MSA_DATABASE_IO                 0x50
#define PMC_ERR_MSA_DATABASE_CORRUPTED          0x51
#define PMC_ERR_MSA_DATABASE_WRITE_FAILED       0x52

/*
 * An array of 16 bytes representing an UUID.
 */
typedef uint8_t pmc_uuid[16];

/*
 * Generic error type, you should usually use this type by defining a null-pointer to it 
 * and passing a pointer to that pointer to any function that accepts it. If an error
 * happens, the function will allocate an error and then write its pointer in the given
 * location. The error should be freed afterward.
 *
 * This structure has a known layout in C, but more information can be accessed using 
 * the `pmc_err_` function, to get the message or the associated data.
 */
typedef struct pmc_err pmc_err;

/*
 * Microsoft Account authenticator.
 */
typedef struct pmc_msa_auth pmc_msa_auth;

/*
 * Microsoft Account device code flow authenticator.
 */
typedef struct pmc_msa_device_code_flow pmc_msa_device_code_flow;

/*
 * Microsoft Account device code flow authenticator.
 */
typedef struct pmc_msa_account pmc_msa_account;

/*
 * A file-backed database for storing accounts.
 */
typedef struct pmc_msa_database pmc_msa_database;

/*
 * A generic function to free any pointer that has been returned by a PortableMC 
 * function, unless explicitly stated.
 */
void                 pmc_free(void *ptr);

/*
 * Retrieve the code of the given error.
 */
uint8_t              pmc_err_code(const pmc_err *err);

/*
 * Return any data associated to the error code, interpretation of that pointer depends
 * on the 
 */
void                *pmc_err_data(const pmc_err *err);

/*
 * Retrieve the message description of the given error.
 */
char                *pmc_err_message(const pmc_err *err);

/*
 * Create a new authenticator with the given application (client) id.
 */
pmc_msa_auth        *pmc_msa_auth_new(const char *app_id);
char                *pmc_msa_auth_app_id(const pmc_msa_auth *auth);
char                *pmc_msa_auth_language_code(const pmc_msa_auth *auth);
void                 pmc_msa_auth_set_language_code(pmc_msa_auth *auth, const char *code);
pmc_msa_device_code_flow *pmc_msa_auth_request_device_code(const pmc_msa_auth *auth, pmc_err **err);

char                *pmc_msa_device_code_flow_app_id(const pmc_msa_device_code_flow *flow);
char                *pmc_msa_device_code_flow_user_code(const pmc_msa_device_code_flow *flow);
char                *pmc_msa_device_code_flow_verification_uri(const pmc_msa_device_code_flow *flow);
char                *pmc_msa_device_code_flow_message(const pmc_msa_device_code_flow *flow);
pmc_msa_account     *pmc_msa_device_code_flow_wait(const pmc_msa_device_code_flow *flow, pmc_err **err);

char                *pmc_msa_account_app_id(const pmc_msa_account *acc);
char                *pmc_msa_account_access_token(const pmc_msa_account *acc);
pmc_uuid            *pmc_msa_account_uuid(const pmc_msa_account *acc);
char                *pmc_msa_account_username(const pmc_msa_account *acc);
char                *pmc_msa_account_xuid(const pmc_msa_account *acc);
void                 pmc_msa_account_request_profile(pmc_msa_account *acc, pmc_err **err);
void                 pmc_msa_account_request_refresh(pmc_msa_account *acc, pmc_err **err);

pmc_msa_database    *pmc_msa_database_new(const char *path);
char                *pmc_msa_database_file(const pmc_msa_database *database);
pmc_msa_account     *pmc_msa_database_load_from_uuid(const pmc_msa_database *database, const pmc_uuid *uuid, pmc_err **err);
pmc_msa_account     *pmc_msa_database_load_from_username(const pmc_msa_database *database, const char *username, pmc_err **err);
pmc_msa_account     *pmc_msa_database_remove_from_uuid(const pmc_msa_database *database, const pmc_uuid *uuid, pmc_err **err);
pmc_msa_account     *pmc_msa_database_remove_from_username(const pmc_msa_database *database, const char *username, pmc_err **err);
void                 pmc_msa_database_store(const pmc_msa_database *database, pmc_msa_account *acc, pmc_err **err);
// TODO: pmc_msa_database_iter

#ifdef __cplusplus
}
#endif

#endif // _PORTABLEMC_H