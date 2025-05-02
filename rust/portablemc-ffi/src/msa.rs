//! MSA bindings for C.

use std::ffi::c_char;
use std::ptr;

use portablemc::msa::{Account, Auth, AuthError, Database, DatabaseError, DeviceCodeFlow};
use uuid::Uuid;

use crate::alloc::{extern_box, extern_box_option, extern_box_cstr_from_str, extern_box_take};
use crate::err::{self, Err, ExposedError, extern_err_with};
use crate::{pmc_uuid, str_from_cstr_ptr};


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

    fn extern_data(&self) -> *mut () {
        match *self {
            AuthError::InvalidStatus(status) => extern_box(status).cast(),
            AuthError::Unknown(ref error) => extern_box_cstr_from_str(error).cast(),
            _ => ptr::null_mut(),
        }
    }

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
pub unsafe extern "C" fn pmc_msa_auth_new(app_id: *const c_char) -> *mut Auth {
    
    let Some(app_id) = (unsafe { str_from_cstr_ptr(app_id) }) else {
        return ptr::null_mut();
    };

    extern_box(Auth::new(app_id))

}

#[no_mangle]
pub unsafe extern "C" fn pmc_msa_auth_app_id(auth: &Auth) -> *mut c_char {
    extern_box_cstr_from_str(auth.app_id())
}

#[no_mangle]
pub unsafe extern "C" fn pmc_msa_auth_language_code(auth: &Auth) -> *mut c_char {
    auth.language_code().map(extern_box_cstr_from_str).unwrap_or(ptr::null_mut())
}

#[no_mangle]
pub unsafe extern "C" fn pmc_msa_auth_set_language_code(auth: &mut Auth, code: *const c_char) {
    
    let Some(code) = (unsafe { str_from_cstr_ptr(code) }) else {
        return;
    };

    auth.set_language_code(code);

}

#[no_mangle]
pub unsafe extern "C" fn pmc_msa_auth_request_device_code(auth: &Auth, err: *mut *mut Err) -> *mut DeviceCodeFlow {
    extern_err_with(err, || {
        auth.request_device_code().map(extern_box)
    }).unwrap_or(ptr::null_mut())
}

// =======
// Binding for DeviceCodeFlow
// =======

#[no_mangle]
pub unsafe extern "C" fn pmc_msa_device_code_flow_app_id(flow: &DeviceCodeFlow) -> *mut c_char {
    extern_box_cstr_from_str(flow.app_id())
}

#[no_mangle]
pub unsafe extern "C" fn pmc_msa_device_code_flow_user_code(flow: &DeviceCodeFlow) -> *mut c_char {
    extern_box_cstr_from_str(flow.user_code())
}

#[no_mangle]
pub unsafe extern "C" fn pmc_msa_device_code_flow_verification_uri(flow: &DeviceCodeFlow) -> *mut c_char {
    extern_box_cstr_from_str(flow.verification_uri())
}

#[no_mangle]
pub unsafe extern "C" fn pmc_msa_device_code_flow_message(flow: &DeviceCodeFlow) -> *mut c_char {
    extern_box_cstr_from_str(flow.message())
}

#[no_mangle]
pub unsafe extern "C" fn pmc_msa_device_code_flow_wait(flow: &DeviceCodeFlow, err: *mut *mut Err) -> *mut Account {
    extern_err_with(err, || {
        flow.wait().map(extern_box)
    }).unwrap_or(ptr::null_mut())
}

// =======
// Binding for Account
// =======

#[no_mangle]
pub unsafe extern "C" fn pmc_msa_account_app_id(acc: &Account) -> *mut c_char {
    extern_box_cstr_from_str(acc.app_id())
}

#[no_mangle]
pub unsafe extern "C" fn pmc_msa_account_access_token(acc: &Account) -> *mut c_char {
    extern_box_cstr_from_str(acc.access_token())
}

#[no_mangle]
pub unsafe extern "C" fn pmc_msa_account_uuid(acc: &Account) -> *mut pmc_uuid {
    extern_box(acc.uuid().as_bytes().clone())
}

#[no_mangle]
pub unsafe extern "C" fn pmc_msa_account_username(acc: &Account) -> *mut c_char {
    extern_box_cstr_from_str(acc.username())
}

#[no_mangle]
pub unsafe extern "C" fn pmc_msa_account_xuid(acc: &Account) -> *mut c_char {
    extern_box_cstr_from_str(acc.xuid())
}

#[no_mangle]
pub unsafe extern "C" fn pmc_msa_account_request_profile(acc: &mut Account, err: *mut *mut Err) {
    let _ = extern_err_with(err, || {
        acc.request_profile()
    });
}

#[no_mangle]
pub unsafe extern "C" fn pmc_msa_account_request_refresh(acc: &mut Account, err: *mut *mut Err) {
    let _ = extern_err_with(err, || {
        acc.request_refresh()
    });
}

// =======
// Binding for Database
// =======

#[no_mangle]
pub unsafe extern "C" fn pmc_msa_database_new(file: *const c_char) -> *mut Database {
    
    let Some(path) = (unsafe { str_from_cstr_ptr(file) }) else {
        return ptr::null_mut();
    };

    extern_box(Database::new(path))

}

#[no_mangle]
pub unsafe extern "C" fn pmc_msa_database_file(db: &Database) -> *mut c_char {
    db.file().as_os_str().to_str().map(extern_box_cstr_from_str).unwrap_or(ptr::null_mut())
}

#[no_mangle]
pub unsafe extern "C" fn pmc_msa_database_load_from_uuid(db: &Database, uuid: *const pmc_uuid, err: *mut *mut Err) -> *mut Account {
    extern_err_with(err, || {
        let uuid = Uuid::from_bytes(unsafe { *uuid });
        db.load_from_uuid(uuid).map(extern_box_option)
    }).unwrap_or(ptr::null_mut())
}

#[no_mangle]
pub unsafe extern "C" fn pmc_msa_database_load_from_username(db: &Database, username: *const c_char, err: *mut *mut Err) -> *mut Account {
    extern_err_with(err, || {

        let Some(username) = (unsafe { str_from_cstr_ptr(username) }) else {
            return Ok(ptr::null_mut());
        };

        db.load_from_username(username).map(extern_box_option)

    }).unwrap_or(ptr::null_mut())
}

#[no_mangle]
pub unsafe extern "C" fn pmc_msa_database_remove_from_uuid(db: &Database, uuid: *const pmc_uuid, err: *mut *mut Err) -> *mut Account {
    extern_err_with(err, || {
        let uuid = Uuid::from_bytes(unsafe { *uuid });
        db.remove_from_uuid(uuid).map(extern_box_option)
    }).unwrap_or(ptr::null_mut())
}

#[no_mangle]
pub unsafe extern "C" fn pmc_msa_database_remove_from_username(db: &Database, username: *const c_char, err: *mut *mut Err) -> *mut Account {
    extern_err_with(err, || {

        let Some(username) = (unsafe { str_from_cstr_ptr(username) }) else {
            return Ok(ptr::null_mut());
        };

        db.remove_from_username(username).map(extern_box_option)

    }).unwrap_or(ptr::null_mut())
}

#[no_mangle]
pub unsafe extern "C" fn pmc_msa_database_store(db: &Database, acc: *mut Account, err: *mut *mut Err) {
    let _ = extern_err_with(err, || {
        let acc = unsafe { extern_box_take(acc) };
        db.store(acc)
    });
}
