use fps_domain::{DockerState, ObservedResources};
use sysinfo::{Disks, System};

pub fn observe() -> ObservedResources {
    let mut sys = System::new();
    sys.refresh_memory();
    sys.refresh_cpu_all();
    let disks = Disks::new_with_refreshed_list();
    let (disk_bytes, disk_available_bytes) = disks.list().iter().fold((0u64, 0u64), |acc, d| {
        (
            acc.0.saturating_add(d.total_space()),
            acc.1.saturating_add(d.available_space()),
        )
    });
    ObservedResources {
        cpu_cores: Some(sys.cpus().len() as u32),
        memory_bytes: Some(sys.total_memory()),
        disk_bytes: Some(disk_bytes),
        disk_available_bytes: Some(disk_available_bytes),
        load_one: Some(sysinfo::System::load_average().one as f32),
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
