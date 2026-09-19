use crate::error::{AppError, Result};

#[cfg(windows)]
const ELEVATED_CHILD_ENVIRONMENT: &str = "CPROF_ELEVATED";
#[cfg(windows)]
const ELEVATED_CHILD_VALUE: &str = "1";

#[cfg(windows)]
pub fn is_elevated_child() -> bool {
    std::env::var(ELEVATED_CHILD_ENVIRONMENT).as_deref() == Ok(ELEVATED_CHILD_VALUE)
}

#[cfg(not(windows))]
pub fn is_elevated_child() -> bool {
    false
}

#[cfg(windows)]
pub fn is_privilege_error(error: &AppError) -> bool {
    error.io_source().is_some_and(|source| {
        source.raw_os_error()
            == Some(windows_sys::Win32::Foundation::ERROR_PRIVILEGE_NOT_HELD as i32)
    })
}

#[cfg(not(windows))]
pub fn is_privilege_error(_error: &AppError) -> bool {
    false
}

/// Re-run this executable with `args` through the Windows `runas` verb.
/// `args` contains command-line arguments without the executable name.
#[cfg(windows)]
pub fn run_as_admin(args: &[std::ffi::OsString]) -> Result<bool> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{GetExitCodeProcess, WaitForSingleObject};
    use windows_sys::Win32::UI::Shell::{
        SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW,
    };

    let executable = std::env::current_exe().map_err(AppError::Io)?;
    let parameters = args.iter().map(quote_arg).collect::<Vec<_>>().join(" ");
    let mut executable = wide(executable.as_os_str());
    let mut parameters = wide(std::ffi::OsStr::new(&parameters));
    let mut runas = wide(std::ffi::OsStr::new("runas"));
    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS,
        hwnd: std::ptr::null_mut(),
        lpVerb: runas.as_mut_ptr(),
        lpFile: executable.as_mut_ptr(),
        lpParameters: parameters.as_mut_ptr(),
        lpDirectory: std::ptr::null_mut(),
        nShow: 1,
        hInstApp: std::ptr::null_mut(),
        lpIDList: std::ptr::null_mut(),
        lpClass: std::ptr::null_mut(),
        hkeyClass: std::ptr::null_mut(),
        dwHotKey: 0,
        Anonymous: unsafe { std::mem::zeroed() },
        hProcess: std::ptr::null_mut(),
    };

    unsafe { std::env::set_var(ELEVATED_CHILD_ENVIRONMENT, ELEVATED_CHILD_VALUE) };
    let launched = unsafe { ShellExecuteExW(&mut info) } != 0;
    unsafe { std::env::remove_var(ELEVATED_CHILD_ENVIRONMENT) };
    if !launched || info.hProcess.is_null() {
        return Err(AppError::ElevationFailed);
    }

    unsafe { WaitForSingleObject(info.hProcess, u32::MAX) };
    let mut exit_code = 1;
    unsafe { GetExitCodeProcess(info.hProcess, &mut exit_code) };
    unsafe { CloseHandle(info.hProcess) };
    Ok(exit_code == 0)
}

#[cfg(not(windows))]
pub fn run_as_admin(_args: &[std::ffi::OsString]) -> Result<bool> {
    Ok(false)
}

#[cfg(windows)]
fn wide(value: &std::ffi::OsStr) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    value.encode_wide().chain(std::iter::once(0)).collect()
}

#[cfg(windows)]
fn quote_arg(value: &std::ffi::OsString) -> String {
    let value = value.to_string_lossy();
    if !value.is_empty() && !value.chars().any(char::is_whitespace) {
        return value.into_owned();
    }

    let mut quoted = String::with_capacity(value.len() + 2);
    quoted.push('"');
    let mut backslashes = 0;
    for character in value.chars() {
        match character {
            '\\' => backslashes += 1,
            '"' => {
                quoted.extend(std::iter::repeat_n('\\', backslashes * 2 + 1));
                quoted.push('"');
                backslashes = 0;
            }
            _ => {
                quoted.extend(std::iter::repeat_n('\\', backslashes));
                quoted.push(character);
                backslashes = 0;
            }
        }
    }
    quoted.extend(std::iter::repeat_n('\\', backslashes * 2));
    quoted.push('"');
    quoted
}

#[cfg(all(test, windows))]
mod tests {
    use super::quote_arg;

    #[test]
    fn quote_arg_preserves_windows_argument_boundaries() {
        assert_eq!(quote_arg(&"profile name".into()), "\"profile name\"");
        assert_eq!(
            quote_arg(&"profile \\\"name".into()),
            "\"profile \\\\\\\"name\""
        );
    }
}
