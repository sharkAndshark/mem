#[cfg(unix)]
mod imp {
    use libc::{getrusage, rusage, RUSAGE_SELF};

    pub fn process_cpu_time_micros() -> Option<u64> {
        let mut usage = rusage {
            ru_utime: libc::timeval {
                tv_sec: 0,
                tv_usec: 0,
            },
            ru_stime: libc::timeval {
                tv_sec: 0,
                tv_usec: 0,
            },
            ru_maxrss: 0,
            ru_ixrss: 0,
            ru_idrss: 0,
            ru_isrss: 0,
            ru_minflt: 0,
            ru_majflt: 0,
            ru_nswap: 0,
            ru_inblock: 0,
            ru_oublock: 0,
            ru_msgsnd: 0,
            ru_msgrcv: 0,
            ru_nsignals: 0,
            ru_nvcsw: 0,
            ru_nivcsw: 0,
        };

        let rc = unsafe { getrusage(RUSAGE_SELF, &mut usage) };
        if rc != 0 {
            return None;
        }

        let user = usage.ru_utime.tv_sec as u64 * 1_000_000 + usage.ru_utime.tv_usec as u64;
        let system = usage.ru_stime.tv_sec as u64 * 1_000_000 + usage.ru_stime.tv_usec as u64;
        Some(user + system)
    }

    pub fn process_private_bytes() -> Option<usize> {
        let pid = std::process::id();
        let output = std::process::Command::new("ps")
            .args(["-p", &pid.to_string(), "-o", "rss="])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let kb = stdout.trim().parse::<usize>().ok()?;
        Some(kb * 1024)
    }
}

#[cfg(target_os = "windows")]
mod imp {
    use std::mem::size_of;
    use windows_sys::Win32::Foundation::GetLastError;
    use windows_sys::Win32::System::ProcessStatus::{
        K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS_EX,
    };
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes, FILETIME};

    fn filetime_to_u64(ft: FILETIME) -> u64 {
        ((ft.dwHighDateTime as u64) << 32) | ft.dwLowDateTime as u64
    }

    pub fn process_cpu_time_micros() -> Option<u64> {
        let process = unsafe { GetCurrentProcess() };
        let mut creation = FILETIME {
            dwLowDateTime: 0,
            dwHighDateTime: 0,
        };
        let mut exit = FILETIME {
            dwLowDateTime: 0,
            dwHighDateTime: 0,
        };
        let mut kernel = FILETIME {
            dwLowDateTime: 0,
            dwHighDateTime: 0,
        };
        let mut user = FILETIME {
            dwLowDateTime: 0,
            dwHighDateTime: 0,
        };

        let ok =
            unsafe { GetProcessTimes(process, &mut creation, &mut exit, &mut kernel, &mut user) };
        if ok == 0 {
            let _ = unsafe { GetLastError() };
            return None;
        }

        let ticks_100ns = filetime_to_u64(kernel) + filetime_to_u64(user);
        Some(ticks_100ns / 10)
    }

    pub fn process_private_bytes() -> Option<usize> {
        let process = unsafe { GetCurrentProcess() };
        let mut counters = PROCESS_MEMORY_COUNTERS_EX {
            cb: size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32,
            PageFaultCount: 0,
            PeakWorkingSetSize: 0,
            WorkingSetSize: 0,
            QuotaPeakPagedPoolUsage: 0,
            QuotaPagedPoolUsage: 0,
            QuotaPeakNonPagedPoolUsage: 0,
            QuotaNonPagedPoolUsage: 0,
            PagefileUsage: 0,
            PeakPagefileUsage: 0,
            PrivateUsage: 0,
        };

        let ok = unsafe {
            K32GetProcessMemoryInfo(
                process,
                &mut counters as *mut PROCESS_MEMORY_COUNTERS_EX as *mut _,
                size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32,
            )
        };
        if ok == 0 {
            return None;
        }

        Some(counters.PrivateUsage)
    }
}

pub fn process_cpu_time_micros() -> Option<u64> {
    imp::process_cpu_time_micros()
}

pub fn process_private_bytes() -> Option<usize> {
    imp::process_private_bytes()
}
