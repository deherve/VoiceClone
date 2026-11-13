//! Les processus enfants (llama-server, whisper-cli) meurent avec l'application : Windows les rattache a un « job » qui
//! les tue des que son dernier handle se ferme, c'est-a-dire a la fin du programme, meme brutale (fermeture de la
//! fenetre, installation d'une mise a jour). Sans cela, un llama-server (2,4 Go) pourrait rester oublie en memoire.
use std::process::Child;

#[cfg(windows)]
mod imp {
    use std::{ffi::c_void, os::windows::io::AsRawHandle, process::Child, sync::OnceLock};

    #[repr(C)]
    struct BasicLimits {
        per_process_user_time: i64,
        per_job_user_time: i64,
        flags: u32,
        min_working_set: usize,
        max_working_set: usize,
        active_process_limit: u32,
        affinity: usize,
        priority_class: u32,
        scheduling_class: u32,
    }

    #[repr(C)]
    struct Extended {
        basic: BasicLimits,
        io_counters: [u64; 6],
        process_memory_limit: usize,
        job_memory_limit: usize,
        peak_process_memory: usize,
        peak_job_memory: usize,
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn CreateJobObjectW(attributes: *const c_void, name: *const u16) -> *mut c_void;
        fn SetInformationJobObject(job: *mut c_void, class: i32, info: *const c_void, len: u32) -> i32;
        fn AssignProcessToJobObject(job: *mut c_void, process: *mut c_void) -> i32;
    }

    const KILL_ON_JOB_CLOSE: u32 = 0x2000;
    const EXTENDED_LIMIT_INFORMATION: i32 = 9;
    static JOB: OnceLock<usize> = OnceLock::new(); // handle du job (0 : creation impossible)

    pub fn adopt(child: &Child) {
        let job = *JOB.get_or_init(|| unsafe {
            let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if job.is_null() {
                return 0;
            }
            let mut info: Extended = std::mem::zeroed();
            info.basic.flags = KILL_ON_JOB_CLOSE;
            if SetInformationJobObject(job, EXTENDED_LIMIT_INFORMATION, &info as *const Extended as *const c_void, std::mem::size_of::<Extended>() as u32) == 0 {
                return 0;
            }
            job as usize
        });
        if job != 0 {
            // shortcut: echec ignore (processus deja termine, ou job imbrique refuse) : l'enfant est alors tue comme avant, par `Drop`.
            unsafe { AssignProcessToJobObject(job as *mut c_void, child.as_raw_handle() as *mut c_void) };
        }
    }
}

/// Rattache `child` au job de l'application (sans effet hors Windows).
pub fn adopt(child: &Child) {
    #[cfg(windows)]
    imp::adopt(child);
    #[cfg(not(windows))]
    let _ = child;
}
