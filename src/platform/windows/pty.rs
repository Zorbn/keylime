use core::str;
use std::{
    ptr::copy_nonoverlapping,
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
};

use windows::{
    core::{Result, HSTRING, PWSTR},
    Win32::{
        Foundation::{CloseHandle, HANDLE},
        Storage::FileSystem::{ReadFile, WriteFile},
        System::{
            Console::{ClosePseudoConsole, CreatePseudoConsole, ResizePseudoConsole, COORD, HPCON},
            Memory::{GetProcessHeap, HeapAlloc, HeapFree, HEAP_FLAGS},
            Pipes::CreatePipe,
            Threading::*,
        },
    },
};

pub struct Pty {
    pub output: Arc<Mutex<Vec<u8>>>,
    pub input: Vec<u8>,

    read_thread_join: Option<JoinHandle<()>>,

    hpcon: HPCON,
    pub(super) hprocess: HANDLE,
    pub(super) event: HANDLE,

    stdin: HANDLE,
}

impl Pty {
    pub fn new(width: usize, height: usize, child_paths: &[&str]) -> Result<Self> {
        // Used to communicate with the child process.
        let mut output_read = HANDLE::default();
        let mut input_write = HANDLE::default();

        let hpcon;
        let event;
        let process_info;

        unsafe {
            // Closed after creating the child process.
            let mut input_read = HANDLE::default();
            let mut output_write = HANDLE::default();

            CreatePipe(&mut input_read, &mut input_write, None, 0)?;
            CreatePipe(&mut output_read, &mut output_write, None, 0)?;

            hpcon = CreatePseudoConsole(
                COORD {
                    X: width as i16,
                    Y: height as i16,
                },
                input_read,
                output_write,
                0,
            )?;

            CloseHandle(input_read)?;
            CloseHandle(output_write)?;

            process_info = Self::create_process(hpcon, child_paths)?;

            event = CreateEventW(None, false, false, None)?;
        }

        let output = Arc::new(Mutex::new(Vec::new()));
        let input = Vec::new();

        let read_thread_join = Self::run_read_thread(output.clone(), output_read, event);

        Ok(Self {
            output,
            input,

            read_thread_join: Some(read_thread_join),

            hpcon,
            hprocess: process_info.hProcess,
            event,

            stdin: input_write,
        })
    }

    unsafe fn create_process(hpcon: HPCON, child_paths: &[&str]) -> Result<PROCESS_INFORMATION> {
        let mut process_info = PROCESS_INFORMATION::default();
        let mut child_result = Ok(());

        let process_heap = GetProcessHeap()?;
        let startup_info = Self::create_process_startup_info(hpcon)?;

        for child_path in child_paths {
            let child_application = HSTRING::from(*child_path);
            let child_application_len = child_application.len() + 1;

            let command = HeapAlloc(
                process_heap,
                HEAP_FLAGS(0),
                size_of::<u16>() * child_application_len,
            );

            copy_nonoverlapping(
                child_application.as_ptr(),
                command as _,
                child_application_len,
            );

            child_result = CreateProcessW(
                None,
                Some(PWSTR(command as _)),
                None,
                None,
                false,
                EXTENDED_STARTUPINFO_PRESENT | CREATE_UNICODE_ENVIRONMENT,
                None,
                None,
                &startup_info.StartupInfo,
                &mut process_info,
            );

            if child_result.is_err() {
                let _ = HeapFree(process_heap, HEAP_FLAGS(0), Some(command));
            } else {
                break;
            }
        }

        child_result
            .inspect_err(|_| {
                let _ = HeapFree(
                    process_heap,
                    HEAP_FLAGS(0),
                    Some(startup_info.lpAttributeList.0),
                );
            })
            .map(|_| process_info)
    }

    unsafe fn create_process_startup_info(hpcon: HPCON) -> Result<STARTUPINFOEXW> {
        let process_heap = GetProcessHeap()?;

        let mut bytes_required = 0;
        let _ = InitializeProcThreadAttributeList(None, 1, None, &mut bytes_required);

        let attribute_list =
            LPPROC_THREAD_ATTRIBUTE_LIST(HeapAlloc(process_heap, HEAP_FLAGS(0), bytes_required));

        InitializeProcThreadAttributeList(Some(attribute_list), 1, None, &mut bytes_required)
            .inspect_err(|_| {
                let _ = HeapFree(process_heap, HEAP_FLAGS(0), Some(attribute_list.0));
            })?;

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

        let mut startup_info = STARTUPINFOEXW::default();
        startup_info.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;
        startup_info.lpAttributeList = attribute_list;

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

impl Drop for Pty {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.event);
            ClosePseudoConsole(self.hpcon);
        }

        let _ = self.read_thread_join.take().unwrap().join();
    }
}
