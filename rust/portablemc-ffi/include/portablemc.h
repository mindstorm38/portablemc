#ifndef _PORTABLEMC_H
#define _PORTABLEMC_H

#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

#ifdef __cplusplus
extern "C" {
#endif

/*
 * Opaque pointer for a standard installer.
 */
typedef struct pmc_standard pmc_standard;

/*
 * Create a new standard installer targeting the given version.
 */
pmc_standard *pmc_standard_new(const char *version);

/*
 * Free the given standard installer, note that this can't be called.
 */
void pmc_standard_free(pmc_standard **inst);

const char *pmc_standard_version(const pmc_standard *inst);
void pmc_standard_set_version(pmc_standard *inst, const char *version);

const char* pmc_standard_versions_dir(const pmc_standard *inst);
void pmc_standard_set_versions_dir(pmc_standard *inst, const char *dir);

/*
 * Opaque pointer for a Mojang installer.
 */
typedef struct pmc_mojang pmc_mojang;

pmc_standard *pmc_mojang_standard();

#ifdef __cplusplus
}
#endif

#endif // _PORTABLEMC_H