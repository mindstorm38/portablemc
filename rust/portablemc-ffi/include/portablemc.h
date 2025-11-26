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

typedef enum pmc_version_type {
    PMC_VERSION_TYPE_NAME,
    PMC_VERSION_TYPE_RELEASE,
    PMC_VERSION_TYPE_SNAPSHOT,
    PMC_VERSION_TYPE_STABLE,
    PMC_VERSION_TYPE_UNSTABLE,
} pmc_version_type;

/*
 * A common structure for specifying a version and the type of version.
 */
typedef struct pmc_version {
    const char *name;
    pmc_version_type type;  // Defaults to 'NAME'.
} pmc_version;

#define PMC_VERSION(name) pmc_version { name, PMC_VERSION_TYPE_NAME }
#define PMC_VERSION_RELEASE pmc_version { NULL, PMC_VERSION_TYPE_RELEASE }
#define PMC_VERSION_SNAPSHOT pmc_version { NULL, PMC_VERSION_TYPE_SNAPSHOT }

/*
 * The installer that supports the minimal standard format for version metadata with
 * support for libraries, assets and loggers automatic installation. By defaults, it 
 * also supports finding a suitable JVM for running the game.
 * 
 * Note that this installer doesn't provide any fetching of missing versions, enables
 * no feature by default and provides no fixes for legacy things. This installer just
 * implements the basics of how Minecraft versions are specified, this is mostly from
 * reverse engineering. Most of the time, you don't want to use this directly, instead
 * you can use the Mojang installer (construct with 'pmc_inst_mojang_new'), that provides 
 * support for fetching missing Mojang versions, various fixes and authentication support.
 */
pmc_inst *pmc_standard_new(const char *version, const char *main_dir);

/*
 * An installer for supporting Mojang-provided versions. It provides support for various
 * standard arguments such as demo mode, window resolution and quick play, it also 
 * provides various fixes for known issues of old versions.
 */
pmc_inst *pmc_mojang_new(pmc_version version, const char *main_dir);

pmc_inst *pmc_fabric_new(const char *main_dir, pmc_fabric_api api);
pmc_inst *pmc_forge_new(const char *main_dir, pmc_forge_api api);

/*
 * Free the given installer, this must be called at most once for each installer, and the
 * pointer must no longer be used afterward.
 */
pmc_inst *pmc_free(pmc_inst **inst);

void pmc_set_root_version(pmc_inst *inst, const char *root_version);
const char *pmc_get_root_version(pmc_inst *inst);

void pmc_install(pmc_inst *inst, pmc_handler *handler);


// Usage example...
int main() {
    pmc_inst *inst = pmc_inst_mojang_new(PMC_VERSION("1.16.5"), NULL);
    pmc_install(inst, NULL);
    pmc_free(&inst);
}

#ifdef __cplusplus
}
#endif

#endif // _PORTABLEMC_H