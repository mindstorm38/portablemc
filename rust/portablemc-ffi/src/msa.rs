//! MSA bindings for C.

use std::ffi::c_char;
use std::ptr;

use portablemc::msa::{Auth, DeviceCodeFlow, Account, AuthError, DatabaseError};

use crate::alloc::extern_box;

use super::err::{self, Err, ExposedError, wrap_error};
use super::alloc::extern_box_cstr_from_str;
use super::cstr::str_from_cstr_ptr;

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

    extern_box(Auth::new(app_id))

}

#[no_mangle]
extern "C" fn pmc_msa_auth_app_id(auth: *const Auth) -> *const c_char {
    let auth = unsafe { &*auth };
    extern_box_cstr_from_str(auth.app_id())
}

#[no_mangle]
extern "C" fn pmc_msa_auth_language_code(auth: *const Auth) -> *const c_char {
    let auth = unsafe { &*auth };
    match auth.language_code() {
        Some(code) => extern_box_cstr_from_str(code),
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
        auth.request_device_code().map(extern_box)
    }, err, ptr::null_mut())
}

// =======
// Binding for DeviceCodeFlow
// =======

#[no_mangle]
extern "C" fn pmc_msa_device_code_flow_app_id(flow: *const DeviceCodeFlow) -> *const c_char {
    let flow = unsafe { &*flow };
    extern_box_cstr_from_str(flow.app_id())
}

#[no_mangle]
extern "C" fn pmc_msa_device_code_flow_user_code(flow: *const DeviceCodeFlow) -> *const c_char {
    let flow = unsafe { &*flow };
    extern_box_cstr_from_str(flow.user_code())
}

#[no_mangle]
extern "C" fn pmc_msa_device_code_flow_verification_uri(flow: *const DeviceCodeFlow) -> *const c_char {
    let flow = unsafe { &*flow };
    extern_box_cstr_from_str(flow.verification_uri())
}

#[no_mangle]
extern "C" fn pmc_msa_device_code_flow_message(flow: *const DeviceCodeFlow) -> *const c_char {
    let flow = unsafe { &*flow };
    extern_box_cstr_from_str(flow.message())
}

#[no_mangle]
extern "C" fn pmc_msa_device_code_flow_wait(flow: *const DeviceCodeFlow, err: *mut *mut Err) -> *mut Account {
    wrap_error(|| {
        let flow = unsafe { &*flow };
        flow.wait().map(extern_box)
    }, err, ptr::null_mut())
}

// =======
// Binding for Account
// =======

#[no_mangle]
extern "C" fn pmc_msa_account_app_id(acc: *const Account) -> *const c_char {
    let account = unsafe { &*acc };
    extern_box_cstr_from_str(account.app_id())
}

#[no_mangle]
extern "C" fn pmc_msa_account_access_token(acc: *const Account) -> *const c_char {
    let account = unsafe { &*acc };
    extern_box_cstr_from_str(account.access_token())
}

#[no_mangle]
extern "C" fn pmc_msa_account_uuid(acc: *const Account) -> *const u8 {
    let account = unsafe { &*acc };
    account.uuid().as_bytes().as_ptr()
}

#[no_mangle]
extern "C" fn pmc_msa_account_username(acc: *const Account) -> *const c_char {
    let account = unsafe { &*acc };
    extern_box_cstr_from_str(account.username())
}

#[no_mangle]
extern "C" fn pmc_msa_account_xuid(acc: *const Account) -> *const c_char {
    let account = unsafe { &*acc };
    extern_box_cstr_from_str(account.xuid())
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

// =======
// Binding for Database
// =======

