// This file 

#include "../../include/portablemc.h"
#include <stdio.h>


void handle_err(pmc_err *err);

int main() {

    pmc_err *err = NULL;

    pmc_msa_auth *auth = pmc_msa_auth_new("appid");

    pmc_msa_device_code_flow *flow = pmc_msa_auth_request_device_code(auth, &err);
    if (err) {
        handle_err(err);
        return 1;
    }

    return 0;

}

void handle_err(pmc_err *err) {

    switch (pmc_err_code(err)) {
        case PMC_ERR_INTERNAL:
            printf("Internal error\n");
            break;
        default:
            char *message = pmc_err_message(err);
            printf("Unhandled error: %s\n", message);
            pmc_free(message);
            break;
    }

}
