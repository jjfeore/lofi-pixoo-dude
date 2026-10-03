//! Pipe ACLs for the current user and logon, including restricted Codex tokens.
use anyhow::{Context, Result, ensure};
use std::{mem, ptr};
use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};
use windows_sys::Win32::{
    Foundation::{CloseHandle, HANDLE, LocalFree},
    Security::{
        Authorization::{
            ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
        },
        GetTokenInformation, SECURITY_ATTRIBUTES, TOKEN_GROUPS, TOKEN_INFORMATION_CLASS,
        TOKEN_QUERY, TOKEN_USER, TokenLogonSid, TokenUser,
    },
    System::Threading::{GetCurrentProcess, OpenProcessToken},
};

struct Token(HANDLE);
impl Drop for Token {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}

fn information(token: HANDLE, class: TOKEN_INFORMATION_CLASS) -> Result<Vec<usize>> {
    let mut needed = 0;
    // Two-call Windows API contract. usize storage provides pointer alignment.
    unsafe {
        GetTokenInformation(token, class, ptr::null_mut(), 0, &mut needed);
    }
    ensure!(
        needed > 0 && needed <= 64 * 1024,
        "invalid Windows token information size"
    );
    let mut bytes = vec![0usize; (needed as usize).div_ceil(mem::size_of::<usize>())];
    ensure!(
        unsafe {
            GetTokenInformation(
                token,
                class,
                bytes.as_mut_ptr().cast(),
                (bytes.len() * mem::size_of::<usize>()) as u32,
                &mut needed,
            )
        } != 0,
        "read Windows token information: {}",
        std::io::Error::last_os_error()
    );
    Ok(bytes)
}

fn sid_string(sid: *mut std::ffi::c_void) -> Result<String> {
    let mut wide = ptr::null_mut();
    ensure!(
        unsafe { ConvertSidToStringSidW(sid, &mut wide) } != 0,
        "convert Windows user identifier"
    );
    // Successful conversion allocates a null-terminated UTF-16 string.
    let value = unsafe {
        let mut len = 0;
        while *wide.add(len) != 0 {
            len += 1;
        }
        let value = String::from_utf16_lossy(std::slice::from_raw_parts(wide, len));
        LocalFree(wide.cast());
        value
    };
    Ok(value)
}

pub fn descriptor() -> Result<String> {
    let mut handle = ptr::null_mut();
    ensure!(
        unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut handle) } != 0,
        "open Windows process token"
    );
    let token = Token(handle);
    let user = information(token.0, TokenUser)?;
    // TokenUser always contains TOKEN_USER; the queried buffer stays alive here.
    let user = unsafe { &*user.as_ptr().cast::<TOKEN_USER>() };
    let user_sid = sid_string(user.User.Sid)?;
    let mut descriptor = format!("D:P(A;;GA;;;{user_sid})(A;;GA;;;SY)");
    let logon = information(token.0, TokenLogonSid)?;
    let groups = unsafe { &*logon.as_ptr().cast::<TOKEN_GROUPS>() };
    if groups.GroupCount > 0 {
        let logon_sid = sid_string(groups.Groups[0].Sid)?;
        // A restricted token also needs an allowed restricted SID. Its logon
        // SID permits this user's session without granting access to Everyone.
        descriptor.push_str(&format!("(A;;GA;;;{logon_sid})"));
    }
    Ok(descriptor)
}

pub fn create(pipe: &str, first: bool, descriptor: &str) -> Result<NamedPipeServer> {
    let wide: Vec<u16> = descriptor.encode_utf16().chain(Some(0)).collect();
    let mut security = ptr::null_mut();
    ensure!(
        unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                wide.as_ptr(),
                1,
                &mut security,
                ptr::null_mut(),
            )
        } != 0,
        "build Windows pipe security descriptor"
    );
    let mut attributes = SECURITY_ATTRIBUTES {
        nLength: mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: security,
        bInheritHandle: 0,
    };
    // Both the attributes and allocated descriptor remain valid for the entire
    // CreateNamedPipe call. Windows copies the descriptor into the pipe object.
    let result = unsafe {
        ServerOptions::new()
            .first_pipe_instance(first)
            .reject_remote_clients(true)
            .create_with_security_attributes_raw(
                pipe,
                (&mut attributes as *mut SECURITY_ATTRIBUTES).cast(),
            )
    };
    unsafe {
        LocalFree(security);
    }
    result.context("create local pipe")
}
