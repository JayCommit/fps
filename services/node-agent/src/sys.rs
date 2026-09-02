use std::thread;
use std::time::Duration;

use fps_domain::{DockerState, ObservedResources};
use sysinfo::{Disks, System};

pub fn observe() -> ObservedResources {
    let mut sys = System::new();
    sys.refresh_memory();
    sys.refresh_cpu_all();
    thread::sleep(Duration::from_millis(150));
    sys.refresh_cpu_all();
    let disks = Disks::new_with_refreshed_list();
    let (disk_bytes, disk_available_bytes) = disks.list().iter().fold((0u64, 0u64), |acc, d| {
        (
            acc.0.saturating_add(d.total_space()),
            acc.1.saturating_add(d.available_space()),
        )
    });
    let memory_bytes = sys.total_memory();
    let memory_used_bytes = sys.used_memory();
    ObservedResources {
        cpu_cores: Some(sys.cpus().len() as u32),
        memory_bytes: Some(memory_bytes),
        memory_used_bytes: Some(memory_used_bytes),
        disk_bytes: Some(disk_bytes),
        disk_available_bytes: Some(disk_available_bytes),
        load_one: Some(sysinfo::System::load_average().one as f32),
        cpu_percent: Some(sys.global_cpu_usage()),
        uptime_seconds: Some(sysinfo::System::uptime()),
    }
}

pub fn docker_unavailable_note() -> &'static str {
    "Docker Engine is not reachable from this agent. Workloads cannot be scheduled here."
}

pub fn docker_state_label(state: DockerState) -> &'static str {
    match state {
        DockerState::Available => "available",
        DockerState::Unavailable => "unavailable",
        DockerState::Error => "error",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observe_reports_host_capacity() {
        let r = observe();
        assert!(r.cpu_cores.unwrap_or(0) >= 1);
        assert!(r.memory_bytes.unwrap_or(0) > 0);
        assert!(r.uptime_seconds.is_some());
    }
}
