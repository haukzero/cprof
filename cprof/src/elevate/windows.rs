use std::env;
use std::ffi::{OsStr, OsString};
use std::iter::once;
use std::mem::size_of;
use std::os::windows::ffi::OsStrExt;
use std::sync::{Mutex, OnceLock};

use windows_sys::Win32::Foundation::{CloseHandle, ERROR_PRIVILEGE_NOT_HELD, WAIT_OBJECT_0};
use windows_sys::Win32::System::Threading::{GetExitCodeProcess, WaitForSingleObject};
use windows_sys::Win32::UI::Shell::{SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW};

use crate::config::home;
use crate::error::{AppError, ElevationError, Result};

use super::arguments::{
    ELEVATED_CONTEXT_ARG, ELEVATED_HOME_ARG, quote_arg, split_context_prefix,
    split_elevation_prefix,
};
use super::context::{ContextFile, read_context};

static ELEVATED: OnceLock<()> = OnceLock::new();
static CONTEXT: Mutex<Option<serde_json::Value>> = Mutex::new(None);

/// ShellExecuteEx can launch under a different account, so carry the original
/// home explicitly instead of depending on inherited environment variables.
pub fn prepare_args(args: Vec<OsString>) -> Result<Vec<OsString>> {
    let (home, args) = split_elevation_prefix(args)?;
    if let Some(home) = home {
        home::set_override(home.clone())?;
        ELEVATED
            .set(())
            .map_err(|_| ElevationError::AlreadyInitialized)?;
        let (reference, args) = split_context_prefix(args)?;
        if let Some(reference) = reference {
            *CONTEXT
                .lock()
                .map_err(|_| ElevationError::AlreadyInitialized)? =
                Some(read_context(&reference, &home, &args)?);
        }
        return Ok(args);
    }
    Ok(args)
}

pub(super) fn take_context() -> Result<Option<serde_json::Value>> {
    Ok(CONTEXT
        .lock()
        .map_err(|_| ElevationError::AlreadyInitialized)?
        .take())
}

pub fn is_elevated_child() -> bool {
    ELEVATED.get().is_some()
}

pub fn is_privilege_error(error: &AppError) -> bool {
    error
        .io_source()
        .is_some_and(|source| source.raw_os_error() == Some(ERROR_PRIVILEGE_NOT_HELD as i32))
}

pub(super) fn run_as_admin(args: &[OsString], context: Option<&serde_json::Value>) -> Result<()> {
    let executable = env::current_exe().map_err(AppError::Io)?;
    let home = home::dir()?;
    let context = context
        .map(|context| ContextFile::new(&home, args, context))
        .transpose()?;
    let mut prefix = vec![OsString::from(ELEVATED_HOME_ARG), home.into_os_string()];
    if let Some(context) = &context {
        let reference = context.reference();
        prefix.extend([
            ELEVATED_CONTEXT_ARG.into(),
            reference.path.into_os_string(),
            reference.digest.into(),
        ]);
    }
    let parameters = prefix
        .iter()
        .chain(args)
        .map(quote_arg)
        .collect::<Vec<_>>()
        .join(" ");
    let executable = wide(executable.as_os_str());
    let parameters = wide(OsStr::new(&parameters));
    let runas = wide(OsStr::new("runas"));
    let mut info = SHELLEXECUTEINFOW {
        cbSize: size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS,
        lpVerb: runas.as_ptr(),
        lpFile: executable.as_ptr(),
        lpParameters: parameters.as_ptr(),
        nShow: 1,
        ..Default::default()
    };

    let launched = unsafe { ShellExecuteExW(&mut info) } != 0;
    if !launched || info.hProcess.is_null() {
        return Err(ElevationError::Failed.into());
    }

    let waited = unsafe { WaitForSingleObject(info.hProcess, u32::MAX) };
    let mut exit_code = 1;
    let read_exit_code = unsafe { GetExitCodeProcess(info.hProcess, &mut exit_code) };
    unsafe { CloseHandle(info.hProcess) };
    if waited != WAIT_OBJECT_0 || read_exit_code == 0 {
        return Err(ElevationError::Failed.into());
    }
    match exit_code {
        0 => Ok(()),
        3 => Err(ElevationError::ReplayMismatch.into()),
        4 => Err(ElevationError::InvalidContext("child rejected the retry context".into()).into()),
        _ => Err(ElevationError::Failed.into()),
    }
}

fn wide(value: &OsStr) -> Vec<u16> {
    value.encode_wide().chain(once(0)).collect()
}
