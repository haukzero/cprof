use std::env;
use std::ffi::{OsStr, OsString};
use std::iter::once;
use std::mem::size_of;
use std::os::windows::ffi::OsStrExt;
use std::sync::OnceLock;

use windows_sys::Win32::Foundation::{CloseHandle, ERROR_PRIVILEGE_NOT_HELD, WAIT_OBJECT_0};
use windows_sys::Win32::System::Threading::{GetExitCodeProcess, WaitForSingleObject};
use windows_sys::Win32::UI::Shell::{SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW};

use crate::config::home;
use crate::error::{AppError, Result};

use super::arguments::{ELEVATED_HOME_ARG, quote_arg, split_elevation_prefix};

static ELEVATED: OnceLock<()> = OnceLock::new();

/// ShellExecuteEx can launch under a different account, so carry the original
/// home explicitly instead of depending on inherited environment variables.
pub fn prepare_args(args: Vec<OsString>) -> Result<Vec<OsString>> {
    let (home, args) = split_elevation_prefix(args)?;
    if let Some(home) = home {
        home::set_override(home)?;
        ELEVATED
            .set(())
            .map_err(|_| AppError::ElevationAlreadyInitialized)?;
    }
    Ok(args)
}

pub fn is_elevated_child() -> bool {
    ELEVATED.get().is_some()
}

pub fn is_privilege_error(error: &AppError) -> bool {
    error
        .io_source()
        .is_some_and(|source| source.raw_os_error() == Some(ERROR_PRIVILEGE_NOT_HELD as i32))
}

pub(super) fn run_as_admin(args: &[OsString]) -> Result<()> {
    let executable = env::current_exe().map_err(AppError::Io)?;
    let prefix = [
        OsString::from(ELEVATED_HOME_ARG),
        home::dir()?.into_os_string(),
    ];
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
        return Err(AppError::ElevationFailed);
    }

    let waited = unsafe { WaitForSingleObject(info.hProcess, u32::MAX) };
    let mut exit_code = 1;
    let read_exit_code = unsafe { GetExitCodeProcess(info.hProcess, &mut exit_code) };
    unsafe { CloseHandle(info.hProcess) };
    if waited == WAIT_OBJECT_0 && read_exit_code != 0 && exit_code == 0 {
        Ok(())
    } else {
        Err(AppError::ElevationFailed)
    }
}

fn wide(value: &OsStr) -> Vec<u16> {
    value.encode_wide().chain(once(0)).collect()
}
