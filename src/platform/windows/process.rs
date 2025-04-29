use core::str;
use std::{
    ptr::copy_nonoverlapping,
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
};

use windows::{
    core::{Result, HSTRING, PWSTR},
    Win32::{
        Foundation::{CloseHandle, SetHandleInformation, HANDLE, HANDLE_FLAG_INHERIT},
        Security::SECURITY_ATTRIBUTES,
        Storage::FileSystem::{ReadFile, WriteFile},
        System::{
            Console::{ClosePseudoConsole, CreatePseudoConsole, ResizePseudoConsole, COORD, HPCON},
            Memory::{GetProcessHeap, HeapAlloc, HeapFree, HEAP_FLAGS},
            Pipes::CreatePipe,
            Threading::*,
        },
    },
};
use windows_core::BOOL;

use crate::platform::process::ProcessKind;

pub struct Process {
    pub output: Arc<Mutex<Vec<u8>>>,
    pub input: Vec<u8>,

    read_thread_join: Option<JoinHandle<()>>,

    hconsole: Option<HPCON>,
    pub(super) hprocess: HANDLE,
    pub(super) event: HANDLE,

    stdin: HANDLE,
    stdout: HANDLE,
}

impl Process {
    pub fn new(commands: &[&str], kind: ProcessKind) -> Result<Self> {
        // Used to communicate with the child process.
        let mut output_read = HANDLE::default();
        let mut input_write = HANDLE::default();

        let hconsole;
        let event;
        let process_info;

        unsafe {
            let security_attributes = SECURITY_ATTRIBUTES {
                nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
                bInheritHandle: BOOL::from(matches!(kind, ProcessKind::Normal)),
                ..Default::default()
            };

            // Closed after creating the child process.
            let mut input_read = HANDLE::default();
            let mut output_write = HANDLE::default();

            CreatePipe(
                &mut input_read,
                &mut input_write,
                Some(&security_attributes),
                0,
            )?;

            CreatePipe(
                &mut output_read,
                &mut output_write,
                Some(&security_attributes),
                0,
            )?;

            SetHandleInformation(output_read, 0, HANDLE_FLAG_INHERIT)?;
            SetHandleInformation(input_write, 0, HANDLE_FLAG_INHERIT)?;

            hconsole = if let ProcessKind::Pty { width, height } = kind {
                Some(CreatePseudoConsole(
                    COORD {
                        X: width as i16,
                        Y: height as i16,
                    },
                    input_read,
                    output_write,
                    0,
                )?)
            } else {
                None
            };

            process_info = Self::create_process(hconsole, input_read, output_write, commands)
                .inspect_err(|_| {
                    let _ = CloseHandle(input_read);
                    let _ = CloseHandle(input_write);
                    let _ = CloseHandle(output_read);
                    let _ = CloseHandle(output_write);

                    if let Some(hconsole) = hconsole {
                        ClosePseudoConsole(hconsole);
                    }
                })?;

            CloseHandle(input_read)?;
            CloseHandle(output_write)?;

            event = CreateEventW(None, false, false, None)?;
        }

        let output = Arc::new(Mutex::new(Vec::new()));
        let input = Vec::new();

        let read_thread_join = Self::run_read_thread(output.clone(), output_read, event);

        Ok(Self {
            output,
            input,

            read_thread_join: Some(read_thread_join),

            hconsole,
            hprocess: process_info.hProcess,
            event,

            stdin: input_write,
            stdout: output_read,
        })
    }

    unsafe fn create_process(
        hconsole: Option<HPCON>,
        input_read: HANDLE,
        output_write: HANDLE,
        commands: &[&str],
    ) -> Result<PROCESS_INFORMATION> {
        let mut process_info = PROCESS_INFORMATION::default();
        let mut result = Ok(());

        let process_heap = GetProcessHeap()?;
        let startup_info = Self::create_process_startup_info(hconsole, input_read, output_write)?;

        for command in commands {
            let wide_command = HSTRING::from(*command);
            let wide_command_len = wide_command.len() + 1;

            let command = HeapAlloc(
                process_heap,
                HEAP_FLAGS(0),
                size_of::<u16>() * wide_command_len,
            );

            copy_nonoverlapping(wide_command.as_ptr(), command as _, wide_command_len);

            result = CreateProcessW(
                None,
                Some(PWSTR(command as _)),
                None,
                None,
                hconsole.is_none(),
                EXTENDED_STARTUPINFO_PRESENT,
                None,
                None,
                &startup_info.StartupInfo,
                &mut process_info,
            );

            if result.is_err() {
                let _ = HeapFree(process_heap, HEAP_FLAGS(0), Some(command));
            } else {
                break;
            }
        }

        result
            .inspect_err(|_| {
                let _ = HeapFree(
                    process_heap,
                    HEAP_FLAGS(0),
                    Some(startup_info.lpAttributeList.0),
                );
            })
            .map(|_| process_info)
    }

    unsafe fn create_process_startup_info(
        hconsole: Option<HPCON>,
        input_read: HANDLE,
        output_write: HANDLE,
    ) -> Result<STARTUPINFOEXW> {
        let attribute_count = if hconsole.is_some() { 1 } else { 0 };

        let process_heap = GetProcessHeap()?;

        let mut bytes_required = 0;
        let _ = InitializeProcThreadAttributeList(None, attribute_count, None, &mut bytes_required);

        let attribute_list =
            LPPROC_THREAD_ATTRIBUTE_LIST(HeapAlloc(process_heap, HEAP_FLAGS(0), bytes_required));

        InitializeProcThreadAttributeList(
            Some(attribute_list),
            attribute_count,
            None,
            &mut bytes_required,
        )
        .inspect_err(|_| {
            let _ = HeapFree(process_heap, HEAP_FLAGS(0), Some(attribute_list.0));
        })?;

        if let Some(hpcon) = hconsole {
            UpdateProcThreadAttribute(
                attribute_list,
                0,
                PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE as usize,
                Some(hpcon.0 as *mut _),
                size_of::<HPCON>(),
                None,
                None,
            )
            .inspect_err(|_| {
                let _ = HeapFree(process_heap, HEAP_FLAGS(0), Some(attribute_list.0));
            })?;
        }

        let mut startup_info = STARTUPINFOEXW::default();
        startup_info.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;
        startup_info.lpAttributeList = attribute_list;

        if hconsole.is_none() {
            startup_info.StartupInfo.hStdOutput = output_write;
            startup_info.StartupInfo.hStdError = output_write;
            startup_info.StartupInfo.hStdInput = input_read;
            startup_info.StartupInfo.dwFlags |= STARTF_USESTDHANDLES;
        }

        Ok(startup_info)
    }

    pub fn flush(&mut self) {
        if self.input.is_empty() {
            return;
        }

        unsafe {
            WriteFile(self.stdin, Some(&self.input), None, None).unwrap();
        }

        self.input.clear();
    }

    pub fn resize(&mut self, width: usize, height: usize) {
        let Some(hpcon) = self.hconsole else {
            return;
        };

        unsafe {
            ResizePseudoConsole(
                hpcon,
                COORD {
                    X: width as i16,
                    Y: height as i16,
                },
            )
            .unwrap();
        }
    }

    fn run_read_thread(
        output: Arc<Mutex<Vec<u8>>>,
        stdout: HANDLE,
        event: HANDLE,
    ) -> JoinHandle<()> {
        let stdout = stdout.0 as usize;
        let event = event.0 as usize;

        thread::spawn(move || {
            let stdout = HANDLE(stdout as _);
            let event = HANDLE(event as _);
            let mut buffer = [0u8; 1024];

            loop {
                let mut bytes_read = 0;

                unsafe {
                    if ReadFile(stdout, Some(&mut buffer), Some(&mut bytes_read), None).is_err() {
                        break;
                    }
                }

                {
                    let mut output = output.lock().unwrap();
                    output.extend_from_slice(&buffer[..bytes_read as usize]);
                }

                unsafe {
                    let _ = SetEvent(event);
                }
            }
        })
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.event);

            if let Some(hconsole) = self.hconsole {
                ClosePseudoConsole(hconsole);
            } else {
                let _ = TerminateProcess(self.hprocess, 0);
            }

            let _ = CloseHandle(self.stdin);
            let _ = CloseHandle(self.stdout);
        }

        if let Some(read_thread_join) = self.read_thread_join.take() {
            let _ = read_thread_join.join();
        }
    }
}
