#ifndef _PORTABLEMC_H
#define _PORTABLEMC_H

#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

#ifdef __cplusplus
extern "C" {
#endif

/*
 * Opaque type representing an abstract installer. The actual installer type behind this
 * depends on the constructor function used.
 */
typedef struct pmc_inst pmc_inst;

typedef struct pmc_handler {
    void (features_loaded)(const char *features[]);
    void (hierarchy_loading)(const char *root_version);
    void (hierarchy_loaded)();
    void (version_loading)(const char *version, const char *file);
    void (version_not_found)(const char *version, const char *file, bool *retry);
    void (version_loaded)(const char *version, const char *file);
    void (client_loading)();
    void (client_loaded)(const char *file);
    void (libraries_loading)();
    void (libraries_loaded)();
} pmc_handler;

typedef enum pmc_fabric_api {
    PMC_FABRIC_API,
    PMC_QUILT_API,
    PMC_LEGACY_FABRIC_API,
    PMC_BABRIC_API,
} pmc_fabric_api;

typedef enum pmc_forge_api {
    PMC_FORGE_API,
    PMC_NEO_FORGE_API,
} pmc_forge_api;

/*
 * Free the given installer, this must be called at most once for each installer, and the
 * pointer must no longer be used afterward.
 */
pmc_inst *pmc_inst_free(pmc_inst **inst);

/*
 * Construct a new installer for standard version.
 */
pmc_inst *pmc_inst_standard_new(const char *root_version, const char *main_dir);
pmc_inst *pmc_inst_standard_new_default(const char *root_version);

pmc_inst *pmc_inst_mojang_new(const char *main_dir);
pmc_inst *pmc_inst_mojang_new_default();

pmc_inst *pmc_inst_fabric_new(const char *main_dir, pmc_fabric_api api);
pmc_inst *pmc_inst_fabric_new_default(pmc_fabric_api api);

pmc_inst *pmc_inst_forge_new(const char *main_dir, pmc_forge_api api);
pmc_inst *pmc_inst_forge_new_default(pmc_forge_api api);

void pmc_set_root_version(pmc_inst *inst, const char *root_version);
const char *pmc_get_root_version(pmc_inst *inst);

void pmc_install(pmc_inst *inst, pmc_handler *handler);


// Usage example...
#if 0
int main() {
    pmc_inst *inst = pmc_inst_standard_new_default("1.21.1");
    pmc_install(inst, NULL);
    pmc_inst_free(&inst);
}
#endif

#ifdef __cplusplus
}
#endif

#endif // _PORTABLEMC_H