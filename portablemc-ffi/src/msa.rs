//! MSA bindings for C.

use std::ptr::{self, NonNull};
use std::ffi::c_char;

use portablemc::msa::{Account, Auth, AuthError, Database, DatabaseError, DeviceCodeFlow};
use uuid::Uuid;

use crate::err::{extern_err_catch, extern_err_static, extern_err, IntoExternErr};
use crate::alloc::{extern_box, extern_cstr_from_str, extern_box_take};
use crate::{cstr, raw};


// =======
// Module errors
// =======

impl IntoExternErr for AuthError {
    
    fn into(self) -> NonNull<raw::pmc_err> {
        use raw::pmc_err_tag::*;
        match self {
            AuthError::Declined => extern_err!(
                PMC_ERR_MSA_AUTH_DECLINED, 
                c"Declined"),
            AuthError::TimedOut => extern_err!(
                PMC_ERR_MSA_AUTH_TIMED_OUT, 
                c"Timed out"),
            AuthError::OutdatedToken => extern_err!(
                PMC_ERR_MSA_AUTH_OUTDATED_TOKEN, 
                c"Minecraft profile token is outdated, you can try to refresh the profile"),
            AuthError::DoesNotOwnGame => extern_err!(
                PMC_ERR_MSA_AUTH_DOES_NOT_OWN_GAME,
                c"This Microsoft account does not own Minecraft"),
            AuthError::InvalidStatus(status) => extern_err!(
                PMC_ERR_MSA_AUTH_INVALID_STATUS,
                format!("An unknown HTTP status has been received: {status}"),
                raw::pmc_err_data_msa_auth_invalid_status {
                    status: status
                }),
            AuthError::Unknown(unknown) => extern_err!(
                PMC_ERR_MSA_AUTH_UNKNOWN, 
                format!("An unknown error happened: {unknown}"),
                raw::pmc_err_data_msa_auth_unknown {
                    message: unknown => cstr
                }),
            AuthError::Internal(error) => extern_err!(
                PMC_ERR_INTERNAL, 
                error.to_string(),
                raw::pmc_err_data_internal {
                    origin: ptr::null()
                }),
            _ => todo!(),
        }
    }

}

impl IntoExternErr for DatabaseError {
    
    fn into(self) -> NonNull<raw::pmc_err> {
        
        use raw::pmc_err_tag::*;

        let (tag, message) = match self {
            DatabaseError::Io(origin) => return extern_err!(
                PMC_ERR_MSA_DATABASE_IO, 
                origin.to_string()),
            DatabaseError::Corrupted => (
                PMC_ERR_MSA_DATABASE_CORRUPTED,
                c"Corrupted"),
            DatabaseError::WriteFailed => (
                PMC_ERR_MSA_DATABASE_WRITE_FAILED,
                c"Failed"),
            _ => todo!(),
        };

        extern_err_static(tag, raw::pmc_err_data::default(), message)

    }

}

// =======
// Binding for Auth
// =======

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pmc_msa_auth_new(app_id: *const c_char) -> NonNull<Auth> {
    let app_id = unsafe { cstr::to_str_lossy(app_id) };
    extern_box(Auth::new(&app_id))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pmc_msa_auth_app_id(auth: &Auth) -> NonNull<c_char> {
    extern_cstr_from_str(auth.app_id())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pmc_msa_auth_language_code(auth: &Auth) -> Option<NonNull<c_char>> {
    auth.language_code().map(extern_cstr_from_str)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pmc_msa_auth_set_language_code(auth: &mut Auth, code: *const c_char) {
    auth.set_language_code(unsafe { cstr::to_str_lossy(code) });
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pmc_msa_auth_request_device_code(auth: &Auth, err: *mut *mut raw::pmc_err) -> Option<NonNull<DeviceCodeFlow>> {
    extern_err_catch(err, || {
        auth.request_device_code().map(extern_box)
    })
}

// =======
// Binding for DeviceCodeFlow
// =======

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pmc_msa_device_code_flow_app_id(flow: &DeviceCodeFlow) -> NonNull<c_char> {
    extern_cstr_from_str(flow.app_id())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pmc_msa_device_code_flow_user_code(flow: &DeviceCodeFlow) -> NonNull<c_char> {
    extern_cstr_from_str(flow.user_code())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pmc_msa_device_code_flow_verification_uri(flow: &DeviceCodeFlow) -> NonNull<c_char> {
    extern_cstr_from_str(flow.verification_uri())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pmc_msa_device_code_flow_message(flow: &DeviceCodeFlow) -> NonNull<c_char> {
    extern_cstr_from_str(flow.message())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pmc_msa_device_code_flow_wait(flow: &DeviceCodeFlow, err: *mut *mut raw::pmc_err) -> Option<NonNull<Account>> {
    extern_err_catch(err, || {
        flow.wait().map(extern_box)
    })
}

// =======
// Binding for Account
// =======

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pmc_msa_account_app_id(acc: &Account) -> NonNull<c_char> {
    extern_cstr_from_str(acc.app_id())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pmc_msa_account_access_token(acc: &Account) -> NonNull<c_char> {
    extern_cstr_from_str(acc.access_token())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pmc_msa_account_uuid(acc: &Account) -> NonNull<raw::pmc_uuid> {
    extern_box(acc.uuid().as_bytes().clone())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pmc_msa_account_username(acc: &Account) -> NonNull<c_char> {
    extern_cstr_from_str(acc.username())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pmc_msa_account_xuid(acc: &Account) -> NonNull<c_char> {
    extern_cstr_from_str(acc.xuid())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pmc_msa_account_request_profile(acc: &mut Account, err: *mut *mut raw::pmc_err) {
    let _ = extern_err_catch(err, || {
        acc.request_profile()
    });
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pmc_msa_account_request_refresh(acc: &mut Account, err: *mut *mut raw::pmc_err) {
    let _ = extern_err_catch(err, || {
        acc.request_refresh()
    });
}

// =======
// Binding for Database
// =======

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pmc_msa_database_new(file: *const c_char) -> NonNull<Database> {
    let file = unsafe { cstr::to_str_lossy(file) }.into_owned();
    extern_box(Database::new(file))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pmc_msa_database_file(db: &Database) -> Option<NonNull<c_char>> {
    db.file().as_os_str().to_str().map(extern_cstr_from_str)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pmc_msa_database_load_from_uuid(db: &Database, uuid: *const raw::pmc_uuid, err: *mut *mut raw::pmc_err) -> Option<NonNull<Account>> {
    extern_err_catch(err, || {
        let uuid = Uuid::from_bytes(unsafe { *uuid });
        db.load_from_uuid(uuid)
    }).unwrap_or(None).map(extern_box)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pmc_msa_database_load_from_username(db: &Database, username: *const c_char, err: *mut *mut raw::pmc_err) -> Option<NonNull<Account>> {
    extern_err_catch(err, || {
        let username = unsafe { cstr::to_str_lossy(username) };
        db.load_from_username(&username)
    }).unwrap_or(None).map(extern_box)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pmc_msa_database_remove_from_uuid(db: &Database, uuid: *const raw::pmc_uuid, err: *mut *mut raw::pmc_err) -> Option<NonNull<Account>> {
    extern_err_catch(err, || {
        let uuid = Uuid::from_bytes(unsafe { *uuid });
        db.remove_from_uuid(uuid)
    }).unwrap_or(None).map(extern_box)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pmc_msa_database_remove_from_username(db: &Database, username: *const c_char, err: *mut *mut raw::pmc_err) -> Option<NonNull<Account>> {
    extern_err_catch(err, || {
        let username = unsafe { cstr::to_str_lossy(username) };
        db.remove_from_username(&username)
    }).unwrap_or(None).map(extern_box)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pmc_msa_database_store(db: &Database, acc: NonNull<Account>, err: *mut *mut raw::pmc_err) {
    let _ = extern_err_catch(err, || {
        let acc = unsafe { extern_box_take(acc) };
        db.store(acc)
    });
}
