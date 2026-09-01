//! In-process fake Proxmox VE API used by bootstrap tests.
//! Mutation endpoints record the request and succeed without creating guests.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{json, Value};

#[derive(Clone, Default)]
pub struct FakeProxmox {
    inner: Arc<Mutex<FakeState>>,
}

#[derive(Default)]
struct FakeState {
    pub posts: Vec<(String, Value)>,
    pub existing_vmids: Vec<u32>,
    pub version: String,
    pub nodes: Vec<String>,
    pub storages: HashMap<String, Vec<String>>,
    pub bridges: HashMap<String, Vec<String>>,
}

impl FakeProxmox {
    pub fn new() -> Self {
        let mut state = FakeState {
            version: "8.3.0".into(),
            nodes: vec!["fry".into(), "homer".into()],
            ..FakeState::default()
        };
        state
            .storages
            .insert("fry".into(), vec!["local".into(), "local-lvm".into()]);
        state
            .storages
            .insert("homer".into(), vec!["local".into(), "local-lvm".into()]);
        state.bridges.insert("fry".into(), vec!["vmbr0".into()]);
        state.bridges.insert("homer".into(), vec!["vmbr0".into()]);
        state.existing_vmids = vec![100, 101];
        Self {
            inner: Arc::new(Mutex::new(state)),
        }
    }

    pub fn recorded_posts(&self) -> Vec<(String, Value)> {
        self.inner.lock().expect("mutex").posts.clone()
    }

    pub fn router(&self) -> Router {
        Router::new()
            .route("/api2/json/version", get(version))
            .route("/api2/json/nodes", get(nodes))
            .route("/api2/json/cluster/nextid", get(nextid))
            .route("/api2/json/cluster/resources", get(cluster_resources))
            .route("/api2/json/nodes/{node}/status", get(node_status))
            .route("/api2/json/nodes/{node}/storage", get(storage))
            .route("/api2/json/nodes/{node}/network", get(network))
            .route("/api2/json/nodes/{node}/lxc", post(create_lxc))
            .route("/api2/json/nodes/{node}/qemu", post(create_qemu))
            .with_state(self.clone())
    }
}

async fn version(State(s): State<FakeProxmox>) -> impl IntoResponse {
    let v = s.inner.lock().expect("mutex").version.clone();
    Json(json!({ "data": { "version": v, "release": v } }))
}

async fn nodes(State(s): State<FakeProxmox>) -> impl IntoResponse {
    let nodes = s.inner.lock().expect("mutex").nodes.clone();
    let data: Vec<Value> = nodes
        .into_iter()
        .map(|node| json!({ "node": node, "status": "online", "cpu": 0.1, "maxcpu": 8, "mem": 8_000_000_000u64, "maxmem": 34_000_000_000u64 }))
        .collect();
    Json(json!({ "data": data }))
}

async fn cluster_resources(State(s): State<FakeProxmox>) -> impl IntoResponse {
    let used = s.inner.lock().expect("mutex").existing_vmids.clone();
    let data: Vec<Value> = used
        .into_iter()
        .map(|vmid| json!({ "vmid": vmid, "type": "qemu", "node": "fry", "status": "running" }))
        .collect();
    Json(json!({ "data": data }))
}

async fn nextid(State(s): State<FakeProxmox>) -> impl IntoResponse {
    let used = s.inner.lock().expect("mutex").existing_vmids.clone();
    let mut id = 120u32;
    while used.contains(&id) {
        id += 1;
    }
    Json(json!({ "data": id.to_string() }))
}

async fn node_status(Path(node): Path<String>, State(s): State<FakeProxmox>) -> impl IntoResponse {
    let known = s.inner.lock().expect("mutex").nodes.clone();
    if !known.contains(&node) {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "errors": "no such node" })),
        )
            .into_response();
    }
    Json(json!({
        "data": {
            "status": "online",
            "uptime": 12_345,
            "cpuinfo": { "cpus": 8 },
            "memory": { "total": 34_359_738_368u64, "used": 8_000_000_000u64, "free": 26_000_000_000u64 },
            "rootfs": { "total": 1_000_000_000_000u64, "avail": 400_000_000_000u64 }
        }
    }))
    .into_response()
}

async fn storage(Path(node): Path<String>, State(s): State<FakeProxmox>) -> impl IntoResponse {
    let storages = s.inner.lock().expect("mutex").storages.get(&node).cloned();
    match storages {
        Some(list) => {
            let data: Vec<Value> = list
                .into_iter()
                .map(|storage| json!({ "storage": storage, "type": "dir", "avail": 400_000_000_000u64, "total": 1_000_000_000_000u64 }))
                .collect();
            Json(json!({ "data": data })).into_response()
        }
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({ "errors": "no such node" })),
        )
            .into_response(),
    }
}

async fn network(Path(node): Path<String>, State(s): State<FakeProxmox>) -> impl IntoResponse {
    let bridges = s.inner.lock().expect("mutex").bridges.get(&node).cloned();
    match bridges {
        Some(list) => {
            let data: Vec<Value> = list
                .into_iter()
                .map(|iface| json!({ "iface": iface, "type": "bridge", "active": 1 }))
                .collect();
            Json(json!({ "data": data })).into_response()
        }
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({ "errors": "no such node" })),
        )
            .into_response(),
    }
}

async fn create_lxc(
    Path(node): Path<String>,
    State(s): State<FakeProxmox>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    s.inner
        .lock()
        .expect("mutex")
        .posts
        .push((format!("/nodes/{node}/lxc"), body.clone()));
    if let Some(vmid) = body.get("vmid").and_then(|v| v.as_u64()) {
        s.inner
            .lock()
            .expect("mutex")
            .existing_vmids
            .push(vmid as u32);
    }
    Json(json!({ "data": "UPID:fake:000:create" }))
}

async fn create_qemu(
    Path(node): Path<String>,
    State(s): State<FakeProxmox>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    s.inner
        .lock()
        .expect("mutex")
        .posts
        .push((format!("/nodes/{node}/qemu"), body.clone()));
    if let Some(vmid) = body.get("vmid").and_then(|v| v.as_u64()) {
        s.inner
            .lock()
            .expect("mutex")
            .existing_vmids
            .push(vmid as u32);
    }
    Json(json!({ "data": "UPID:fake:000:create" }))
}
