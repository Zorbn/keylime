use std::{
    ffi::c_void,
    ptr::{copy_nonoverlapping, null_mut},
};

use windows::{
    core::{w, Result, PWSTR},
    Win32::{
        Foundation::{CloseHandle, FALSE, HANDLE},
        System::{
            Console::{ClosePseudoConsole, CreatePseudoConsole, ResizePseudoConsole, COORD, HPCON},
            Memory::{GetProcessHeap, HeapAlloc, HeapFree, HEAP_FLAGS},
            Pipes::CreatePipe,
            Threading::*,
        },
    },
};

pub struct Pty {
    hpcon: HPCON,
    hprocess: HANDLE,
    stdout: HANDLE,
    stdin: HANDLE,
}

impl Pty {
    pub fn new(width: isize, height: isize) -> Result<Self> {
        // Closed after creating the child process.
        let mut input_read = HANDLE::default();
        let mut output_write = HANDLE::default();

        // Used to communicate with the child process.
        let mut output_read = HANDLE::default();
        let mut input_write = HANDLE::default();

        unsafe {
            CreatePipe(&mut input_read, &mut input_write, None, 0)?;
            CreatePipe(&mut output_read, &mut output_write, None, 0)?;

            CreatePseudoConsole(
                COORD {
                    X: width as i16,
                    Y: height as i16,
                },
                input_read,
                output_write,
                0,
            )?;

            let mut bytes_required = 0;
            let _ = InitializeProcThreadAttributeList(
                LPPROC_THREAD_ATTRIBUTE_LIST(null_mut()),
                1,
                0,
                &mut bytes_required,
            );

            let process_heap = GetProcessHeap()?;

            let attribute_list = LPPROC_THREAD_ATTRIBUTE_LIST(HeapAlloc(
                GetProcessHeap()?,
                HEAP_FLAGS(0),
                bytes_required,
            ));

            InitializeProcThreadAttributeList(attribute_list, 1, 0, &mut bytes_required)
                .inspect_err(|_| {
                    let _ = HeapFree(process_heap, HEAP_FLAGS(0), Some(attribute_list.0));
                })?;

            let hpcon = HPCON::default();

            UpdateProcThreadAttribute(
                attribute_list,
                0,
                PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE as usize,
                Some(&hpcon as *const _ as *const c_void),
                size_of::<HPCON>(),
                None,
                None,
            )
            .inspect_err(|_| {
                let _ = HeapFree(process_heap, HEAP_FLAGS(0), Some(attribute_list.0));
            })?;

            let mut startup_info = STARTUPINFOEXW::default();
            startup_info.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;
            startup_info.lpAttributeList = attribute_list;

            let child_application = w!("C:\\Program Files\\PowerShell\\7\\pwsh.exe");
            let child_application_len = child_application.len() + 1;

            let command = HeapAlloc(
                process_heap,
                HEAP_FLAGS(0),
                size_of::<u16>() * child_application_len,
            );

            copy_nonoverlapping(child_application.0, command as _, child_application_len);

            let mut process_info = PROCESS_INFORMATION::default();

            CreateProcessW(
                None,
                PWSTR(command as _),
                None,
                None,
                FALSE,
                EXTENDED_STARTUPINFO_PRESENT,
                None,
                None,
                &startup_info.StartupInfo,
                &mut process_info,
            )
            .inspect_err(|_| {
                let _ = HeapFree(process_heap, HEAP_FLAGS(0), Some(attribute_list.0));
            })?;

            CloseHandle(input_read)?;
            CloseHandle(output_write)?;

            Ok(Self {
                hpcon,
                hprocess: process_info.hProcess,
                stdout: output_read,
                stdin: input_write,
            })
        }
    }

    pub fn resize(&mut self, width: isize, height: isize) {
        unsafe {
            ResizePseudoConsole(
                self.hpcon,
                COORD {
                    X: width as i16,
                    Y: height as i16,
                },
            )
            .unwrap();
        }
    }
}

impl Drop for Pty {
    fn drop(&mut self) {
        unsafe {
            ClosePseudoConsole(self.hpcon);
        }
    }
}
