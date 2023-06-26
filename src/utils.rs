use sysinfo::{PidExt, ProcessExt, System, SystemExt};

/// Given an **exact** process name, it returns its PID, if available.
pub fn process_pid_by_name(process_name: &str) -> Option<u32> {
    let system = System::new_all();
    let mut processes = system.processes_by_exact_name(process_name);
    (*processes).next().map(|process| process.pid().as_u32())
}