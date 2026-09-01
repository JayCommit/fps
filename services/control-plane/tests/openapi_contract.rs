use std::path::PathBuf;

#[test]
fn permissions_dump_contains_canonical_ids() {
    let dumped: Vec<String> = serde_json::from_str(&fps_control_plane::dump_permissions()).unwrap();
    assert!(dumped.contains(&"nodes.enroll".into()));
    assert!(dumped.contains(&"platform.setup".into()));
}

#[test]
fn openapi_is_nonempty_v1() {
    let dumped = fps_control_plane::dump_openapi();
    assert!(dumped.contains("\"openapi\": \"3.1.0\""));
    assert!(dumped.contains("/v1/nodes/enroll"));
    let candidates = [
        PathBuf::from("packages/api-client-generated/openapi.json"),
        PathBuf::from("../../packages/api-client-generated/openapi.json"),
    ];
    if let Some(path) = candidates.iter().find(|p| p.exists()) {
        let committed = std::fs::read_to_string(path).unwrap();
        assert_eq!(dumped.trim(), committed.trim());
    }
}
