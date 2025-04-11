//! MSA bindings for C.

use std::ffi::{CStr, c_char};
use std::ptr::{self, NonNull};
use std::borrow::Cow;

use portablemc::msa::{Auth, DeviceCodeFlow, Account, Database, AuthError, DatabaseError};

use super::err::{self, Err, ExposedError, ExposedErrorData, wrap_error};
use super::ffi::str_from_cstr_ptr;


// =======
// Module errors
// =======

impl ExposedError for AuthError {

    fn code(&self) -> u8 {
        match self {
            AuthError::Declined => err::code::MSA_AUTH_DECLINED,
            AuthError::TimedOut => err::code::MSA_AUTH_TIMED_OUT,
            AuthError::OutdatedToken => err::code::MSA_AUTH_OUTDATED_TOKEN,
            AuthError::DoesNotOwnGame => err::code::MSA_AUTH_DOES_NOT_OWN_GAME,
            AuthError::InvalidStatus(_) => err::code::MSA_AUTH_INVALID_STATUS,
            AuthError::Unknown(_) => err::code::MSA_AUTH_UNKNOWN,
            AuthError::Internal(_) => err::code::INTERNAL,
            _ => todo!(),
        }
    }

    // fn data(&self) -> Option<Box<dyn ExposedErrorData>> {
    //     Some(match self {
    //         AuthError::InvalidStatus(status) => Box::new(*status),
    //         AuthError::Unknown(error) => Box::new()
    //         _ => return None,
    //     })
    // }

}

impl ExposedError for DatabaseError {
    fn code(&self) -> u8 {
        match self {
            DatabaseError::Io(_) => err::code::MSA_DATABASE_IO,
            DatabaseError::Corrupted => err::code::MSA_DATABASE_CORRUPTED,
            DatabaseError::WriteFailed => err::code::MSA_DATABASE_WRITE_FAILED,
            _ => todo!(),
        }
    }
}

// =======
// Binding for Auth
// =======

#[no_mangle]
extern "C" fn pmc_msa_auth_new(app_id: *const c_char) -> *mut Auth {
    
    let Some(app_id) = (unsafe { str_from_cstr_ptr(app_id) }) else {
        return ptr::null_mut();
    };

    Box::into_raw(Box::new(Auth::new(app_id)))

}

#[no_mangle]
extern "C" fn pmc_msa_auth_app_id(auth: *const Auth) -> *const c_char {
    let auth = unsafe { &*auth };
    // FIXME: Returning nul-terminated str
    auth.app_id().as_ptr().cast()
}

#[no_mangle]
extern "C" fn pmc_msa_auth_language_code(auth: *const Auth) -> *const c_char {
    let auth = unsafe { &*auth };
    match auth.language_code() {
        Some(code) => code.as_ptr().cast(),
        None => ptr::null(),
    }
}

#[no_mangle]
extern "C" fn pmc_msa_auth_set_language_code(auth: *mut Auth, code: *const c_char) {
    
    let auth = unsafe { &mut *auth };
    
    let Some(code) = (unsafe { str_from_cstr_ptr(code) }) else {
        return;
    };

    auth.set_language_code(code);

}

#[no_mangle]
extern "C" fn pmc_msa_auth_request_device_code(auth: *const Auth, err: *mut *mut Err) -> *mut DeviceCodeFlow {
    wrap_error(|| {
        let auth = unsafe { &*auth };
        auth.request_device_code().map(|flow| Box::into_raw(Box::new(flow)))
    }, err, ptr::null_mut())
}

#[no_mangle]
extern "C" fn pmc_msa_auth_free(auth: *mut Auth) {
    // SAFETY: Should've been allocated as a Box.
    unsafe { drop(Box::from_raw(auth)) }
}

// =======
// Binding for DeviceCodeFlow
// =======

#[no_mangle]
extern "C" fn pmc_msa_device_code_flow_app_id(flow: *const DeviceCodeFlow) -> *const c_char {
    let flow = unsafe { &*flow };
    flow.app_id().as_ptr().cast()
}

#[no_mangle]
extern "C" fn pmc_msa_device_code_flow_user_code(flow: *const DeviceCodeFlow) -> *const c_char {
    let flow = unsafe { &*flow };
    flow.user_code().as_ptr().cast()
}

#[no_mangle]
extern "C" fn pmc_msa_device_code_flow_verification_uri(flow: *const DeviceCodeFlow) -> *const c_char {
    let flow = unsafe { &*flow };
    flow.verification_uri().as_ptr().cast()
}

#[no_mangle]
extern "C" fn pmc_msa_device_code_flow_message(flow: *const DeviceCodeFlow) -> *const c_char {
    let flow = unsafe { &*flow };
    flow.message().as_ptr().cast()
}

#[no_mangle]
extern "C" fn pmc_msa_device_code_flow_wait(flow: *const DeviceCodeFlow, err: *mut *mut Err) -> *mut Account {
    wrap_error(|| {
        let flow = unsafe { &*flow };
        flow.wait().map(|acc| Box::into_raw(Box::new(acc)))
    }, err, ptr::null_mut())
}

#[no_mangle]
extern "C" fn pmc_msa_device_code_flow_free(flow: *mut DeviceCodeFlow) {
    // SAFETY: Should've been allocated as a Box.
    unsafe { drop(Box::from_raw(flow)) }
}

// =======
// Binding for Account
// =======

#[no_mangle]
extern "C" fn pmc_msa_account_app_id(acc: *const Account) -> *const c_char {
    let account = unsafe { &*acc };
    account.app_id().as_ptr().cast()
}

#[no_mangle]
extern "C" fn pmc_msa_account_access_token(acc: *const Account) -> *const c_char {
    let account = unsafe { &*acc };
    account.access_token().as_ptr().cast()
}

#[no_mangle]
extern "C" fn pmc_msa_account_uuid(acc: *const Account) -> *const u8 {
    let account = unsafe { &*acc };
    account.uuid().as_bytes().as_ptr()
}

#[no_mangle]
extern "C" fn pmc_msa_account_username(acc: *const Account) -> *const c_char {
    let account = unsafe { &*acc };
    account.username().as_ptr().cast()
}

#[no_mangle]
extern "C" fn pmc_msa_account_xuid(acc: *const Account) -> *const c_char {
    let account = unsafe { &*acc };
    account.xuid().as_ptr().cast()
}

#[no_mangle]
extern "C" fn pmc_msa_account_request_profile(acc: *mut Account, err: *mut *mut Err) {
    wrap_error(|| {
        let account = unsafe { &mut *acc };
        account.request_profile()
    }, err, ())
}

#[no_mangle]
extern "C" fn pmc_msa_account_request_refresh(acc: *mut Account, err: *mut *mut Err) {
    wrap_error(|| {
        let account = unsafe { &mut *acc };
        account.request_refresh()
    }, err, ())
} 

#[no_mangle]
extern "C" fn pmc_msa_account_free(acc: *mut Account) {
    // SAFETY: Should've been allocated as a Box.
    unsafe { drop(Box::from_raw(acc)) }
}

// =======
// Binding for Database
// =======

