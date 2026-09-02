use std::collections::{BTreeMap, HashSet};

use chrono::{DateTime, Utc};
use fps_domain::{AllocationId, NodeId, PortMapping, ServerId};
use sqlx::MySqlPool;

use super::decode::parse_id;
use super::{from_naive, now_utc};

/// Last-resort scan when the game's own port and everything above it is taken.
pub const FALLBACK_RANGE_START: i32 = 25000;
pub const FALLBACK_RANGE_END: i32 = 25999;

pub struct AllocationRecord {
    pub id: AllocationId,
    pub node_id: NodeId,
    pub ip: String,
    pub port: i32,
    pub protocol: String,
    pub assigned_server_id: Option<ServerId>,
    pub created_at: DateTime<Utc>,
}

/// One published mapping after host-port assignment.
#[derive(Debug, Clone)]
pub struct AllocatedBinding {
    pub allocation_id: AllocationId,
    pub name: String,
    pub protocol: String,
    pub container_port: i32,
    pub host_port: i32,
    pub ip: String,
}

/// Pick a host port for a group of protocols that must share the same number
/// (for example CS2 27015/tcp + 27015/udp). Prefers the game's default port.
pub fn pick_host_port(
    preferred: i32,
    protocols: &[&str],
    used: &HashSet<(i32, String)>,
) -> Option<i32> {
    let preferred = preferred.clamp(1, 65535);
    if port_free(preferred, protocols, used) {
        return Some(preferred);
    }
    (preferred + 1..=65535)
        .chain(FALLBACK_RANGE_START..=FALLBACK_RANGE_END)
        .chain(1024..preferred)
        .find(|port| port_free(*port, protocols, used))
}

fn port_free(port: i32, protocols: &[&str], used: &HashSet<(i32, String)>) -> bool {
    protocols
        .iter()
        .all(|proto| !used.contains(&(port, proto.to_ascii_lowercase())))
}

pub fn is_port_bind_conflict(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("port is already allocated")
        || lower.contains("address already in use")
        || (lower.contains("bind for") && lower.contains("failed"))
}

pub fn parse_conflict_host_port(message: &str) -> Option<i32> {
    const NEEDLE: &str = "Bind for ";
    if let Some(start) = message.find(NEEDLE) {
        let rest = &message[start + NEEDLE.len()..];
        if let Some(addr) = rest.split_whitespace().next() {
            if let Some((_, port)) = addr.rsplit_once(':') {
                if let Ok(parsed) = port.parse::<i32>() {
                    if (1..=65535).contains(&parsed) {
                        return Some(parsed);
                    }
                }
            }
        }
    }
    if let Some(rest) = message.strip_prefix("Host port ") {
        if let Some(digits) = rest.split_whitespace().next() {
            if let Ok(parsed) = digits.parse::<i32>() {
                if (1..=65535).contains(&parsed) {
                    return Some(parsed);
                }
            }
        }
    }
    let lower = message.to_ascii_lowercase();
    if let Some(idx) = lower.find("0.0.0.0:") {
        let digits: String = message[idx + "0.0.0.0:".len()..]
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect();
        if let Ok(parsed) = digits.parse::<i32>() {
            if (1..=65535).contains(&parsed) {
                return Some(parsed);
            }
        }
    }
    None
}

pub async fn allocate_for_ports(
    pool: &MySqlPool,
    node_id: NodeId,
    ports: &[PortMapping],
) -> Result<Vec<AllocatedBinding>, sqlx::Error> {
    if ports.is_empty() {
        return Ok(Vec::new());
    }
    let mut used = used_binds(pool, node_id).await?;
    let mut groups: BTreeMap<i32, Vec<&PortMapping>> = BTreeMap::new();
    for port in ports {
        groups
            .entry(i32::from(port.container_port))
            .or_default()
            .push(port);
    }
    let mut out = Vec::new();
    for (container_port, group) in groups {
        let protocols: Vec<String> = group
            .iter()
            .map(|p| normalize_protocol(&p.protocol))
            .collect();
        let proto_refs: Vec<&str> = protocols.iter().map(String::as_str).collect();
        let host_port = pick_host_port(container_port, &proto_refs, &used).ok_or_else(|| {
            sqlx::Error::Protocol("no free host ports on node for this game".into())
        })?;
        for mapping in group {
            let protocol = normalize_protocol(&mapping.protocol);
            let rec = insert_bind_named(
                pool,
                node_id,
                host_port,
                &protocol,
                &notes_for(&mapping.name, container_port),
            )
            .await?;
            used.insert((host_port, protocol.clone()));
            out.push(AllocatedBinding {
                allocation_id: rec.id,
                name: mapping.name.clone(),
                protocol,
                container_port,
                host_port,
                ip: rec.ip,
            });
        }
    }
    Ok(out)
}

pub async fn assign_server(
    pool: &MySqlPool,
    id: AllocationId,
    server_id: ServerId,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE allocations SET assigned_server_id = ? WHERE id = ?")
        .bind(server_id.to_string())
        .bind(id.to_string())
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn assign_all(
    pool: &MySqlPool,
    server_id: ServerId,
    ids: &[AllocationId],
) -> Result<(), sqlx::Error> {
    for id in ids {
        assign_server(pool, *id, server_id).await?;
    }
    Ok(())
}

/// `notes` stores `name|container_port`. The `port` column is the host bind.
pub async fn list_bindings_for_server(
    pool: &MySqlPool,
    server_id: ServerId,
) -> Result<Vec<AllocatedBinding>, sqlx::Error> {
    let rows: Vec<(String, String, i32, String, Option<String>)> = sqlx::query_as(
        "SELECT id, ip, port, protocol, notes FROM allocations
         WHERE assigned_server_id = ? ORDER BY port, protocol",
    )
    .bind(server_id.to_string())
    .fetch_all(pool)
    .await?;
    rows.into_iter()
        .map(|(id, ip, host_port, protocol, notes)| {
            let (name, container_port) = parse_notes(notes.as_deref(), host_port);
            Ok(AllocatedBinding {
                allocation_id: parse_id(&id, "allocations.id")?,
                name,
                protocol,
                container_port,
                host_port,
                ip,
            })
        })
        .collect()
}

fn parse_notes(notes: Option<&str>, fallback_port: i32) -> (String, i32) {
    let Some(notes) = notes.filter(|s| !s.is_empty()) else {
        return ("game".into(), fallback_port);
    };
    if let Some((name, rest)) = notes.split_once('|') {
        let container = rest.parse::<i32>().unwrap_or(fallback_port);
        return (name.to_string(), container);
    }
    (notes.to_string(), fallback_port)
}

pub fn notes_for(name: &str, container_port: i32) -> String {
    format!("{name}|{container_port}")
}

pub async fn release_server(pool: &MySqlPool, server_id: ServerId) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM allocations WHERE assigned_server_id = ?")
        .bind(server_id.to_string())
        .execute(pool)
        .await?;
    Ok(())
}

/// After Docker reports a host port is taken outside FPS, keep that bind
/// reserved and pick a new host port for the server's mapping(s) on it.
pub async fn reallocate_blocked_port(
    pool: &MySqlPool,
    node_id: NodeId,
    server_id: ServerId,
    blocked_host_port: i32,
) -> Result<Vec<AllocatedBinding>, sqlx::Error> {
    let current = list_bindings_for_server(pool, server_id).await?;
    let blocked: Vec<AllocatedBinding> = current
        .iter()
        .filter(|b| b.host_port == blocked_host_port)
        .cloned()
        .collect();
    if blocked.is_empty() {
        return Ok(current);
    }
    for bind in &blocked {
        sqlx::query("UPDATE allocations SET assigned_server_id = NULL, notes = ? WHERE id = ?")
            .bind(format!(
                "blocked on host (port {blocked_host_port} already allocated)"
            ))
            .bind(bind.allocation_id.to_string())
            .execute(pool)
            .await?;
    }
    let mut used = used_binds(pool, node_id).await?;
    let protocols: Vec<String> = blocked.iter().map(|b| b.protocol.clone()).collect();
    let proto_refs: Vec<&str> = protocols.iter().map(String::as_str).collect();
    let preferred = blocked
        .first()
        .map(|b| b.container_port)
        .unwrap_or(blocked_host_port);
    let host_port = pick_host_port(preferred, &proto_refs, &used).ok_or_else(|| {
        sqlx::Error::Protocol("no free host ports on node after bind conflict".into())
    })?;
    for bind in &blocked {
        let rec = insert_bind_named(
            pool,
            node_id,
            host_port,
            &bind.protocol,
            &notes_for(&bind.name, bind.container_port),
        )
        .await?;
        assign_server(pool, rec.id, server_id).await?;
        used.insert((host_port, bind.protocol.clone()));
    }
    list_bindings_for_server(pool, server_id).await
}

async fn used_binds(
    pool: &MySqlPool,
    node_id: NodeId,
) -> Result<HashSet<(i32, String)>, sqlx::Error> {
    let rows: Vec<(i32, String)> =
        sqlx::query_as("SELECT port, protocol FROM allocations WHERE node_id = ?")
            .bind(node_id.to_string())
            .fetch_all(pool)
            .await?;
    Ok(rows
        .into_iter()
        .map(|(port, proto)| (port, proto.to_ascii_lowercase()))
        .collect())
}

async fn insert_bind_named(
    pool: &MySqlPool,
    node_id: NodeId,
    port: i32,
    protocol: &str,
    notes: &str,
) -> Result<AllocationRecord, sqlx::Error> {
    let id = AllocationId::new();
    let now = now_utc();
    let notes = if notes.is_empty() { None } else { Some(notes) };
    sqlx::query(
        "INSERT INTO allocations (id, node_id, ip, port, protocol, notes, assigned_server_id, created_at)
         VALUES (?, ?, '0.0.0.0', ?, ?, ?, NULL, ?)",
    )
    .bind(id.to_string())
    .bind(node_id.to_string())
    .bind(port)
    .bind(protocol)
    .bind(notes)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(AllocationRecord {
        id,
        node_id,
        ip: "0.0.0.0".into(),
        port,
        protocol: protocol.into(),
        assigned_server_id: None,
        created_at: from_naive(now),
    })
}

fn normalize_protocol(protocol: &str) -> String {
    let p = protocol.trim().to_ascii_lowercase();
    if p.is_empty() {
        "tcp".into()
    } else {
        p
    }
}

#[allow(clippy::type_complexity)]
pub async fn get(
    pool: &MySqlPool,
    id: AllocationId,
) -> Result<Option<AllocationRecord>, sqlx::Error> {
    let row: Option<(
        String,
        String,
        String,
        i32,
        String,
        Option<String>,
        chrono::NaiveDateTime,
    )> = sqlx::query_as(
        "SELECT id, node_id, ip, port, protocol, assigned_server_id, created_at FROM allocations WHERE id = ?",
    )
    .bind(id.to_string())
    .fetch_optional(pool)
    .await?;
    row.map(|(id, node_id, ip, port, protocol, assigned, created_at)| {
        Ok(AllocationRecord {
            id: parse_id(&id, "allocations.id")?,
            node_id: parse_id(&node_id, "allocations.node_id")?,
            ip,
            port,
            protocol,
            assigned_server_id: assigned
                .map(|s| parse_id(&s, "allocations.assigned_server_id"))
                .transpose()?,
            created_at: from_naive(created_at),
        })
    })
    .transpose()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefers_the_game_default_port() {
        let used = HashSet::new();
        assert_eq!(pick_host_port(25565, &["tcp"], &used), Some(25565));
        assert_eq!(pick_host_port(27015, &["tcp", "udp"], &used), Some(27015));
    }

    #[test]
    fn skips_taken_default_and_keeps_tcp_udp_together() {
        let mut used = HashSet::new();
        used.insert((27015, "tcp".into()));
        assert_eq!(pick_host_port(27015, &["tcp", "udp"], &used), Some(27016));
        used.insert((27016, "udp".into()));
        assert_eq!(pick_host_port(27015, &["tcp", "udp"], &used), Some(27017));
    }

    #[test]
    fn parses_docker_bind_conflict() {
        let msg = "Docker responded with status code 500: failed to set up container networking: \
                   driver failed programming external connectivity on endpoint fps-01a062b3 \
                   (8946a384aa802cc3ca216199806131c4363b243a889712065cd264346fda2534): \
                   Bind for 0.0.0.0:25000 failed: port is already allocated";
        assert!(is_port_bind_conflict(msg));
        assert_eq!(parse_conflict_host_port(msg), Some(25000));
        assert_eq!(
            parse_conflict_host_port("Host port 27015 is already in use on this node."),
            Some(27015)
        );
    }

    #[test]
    fn notes_roundtrip() {
        assert_eq!(
            parse_notes(Some("game-udp|27015"), 25000),
            ("game-udp".into(), 27015)
        );
        assert_eq!(parse_notes(Some("rcon"), 28016), ("rcon".into(), 28016));
    }
}
