use anyhow::{Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde_json::Value;

/// Read-only + mutation surface used by bootstrap. Fake and real clients share it.
#[async_trait]
pub trait ProxmoxView: Send + Sync {
    async fn version(&self) -> Result<String>;
    async fn node_online(&self, node: &str) -> Result<bool>;
    async fn has_storage(&self, node: &str, storage: &str) -> Result<bool>;
    async fn has_bridge(&self, node: &str, iface: &str) -> Result<bool>;
    async fn vmid_in_use(&self, vmid: u32) -> Result<bool>;
    async fn create_lxc(&self, node: &str, body: Value) -> Result<String>;
    async fn create_qemu(&self, node: &str, body: Value) -> Result<String>;
}

pub struct ProxmoxClient {
    base: String,
    token_id: String,
    token_secret: String,
    client: Client,
}

impl ProxmoxClient {
    pub fn new(base: &str, token_id: &str, token_secret: &str, insecure: bool) -> Result<Self> {
        let client = Client::builder()
            .danger_accept_invalid_certs(insecure)
            .user_agent(fps_branding::user_agent())
            .build()?;
        Ok(Self {
            base: base.trim_end_matches('/').to_string(),
            token_id: token_id.to_string(),
            token_secret: token_secret.to_string(),
            client,
        })
    }

    async fn get(&self, path: &str) -> Result<Value> {
        let url = format!("{}{path}", self.base);
        let res = self
            .client
            .get(url)
            .header(
                "Authorization",
                format!("PVEAPIToken={}:{}", self.token_id, self.token_secret),
            )
            .send()
            .await
            .context("proxmox GET")?;
        let status = res.status();
        let body = res.json::<Value>().await.unwrap_or(Value::Null);
        if !status.is_success() {
            anyhow::bail!("proxmox {path} -> {status} {body}");
        }
        Ok(body)
    }

    async fn post(&self, path: &str, body: Value) -> Result<Value> {
        let url = format!("{}{path}", self.base);
        let res = self
            .client
            .post(url)
            .header(
                "Authorization",
                format!("PVEAPIToken={}:{}", self.token_id, self.token_secret),
            )
            .json(&body)
            .send()
            .await
            .context("proxmox POST")?;
        let status = res.status();
        let body = res.json::<Value>().await.unwrap_or(Value::Null);
        if !status.is_success() {
            anyhow::bail!("proxmox {path} -> {status}");
        }
        Ok(body)
    }
}

#[async_trait]
impl ProxmoxView for ProxmoxClient {
    async fn version(&self) -> Result<String> {
        let body = self.get("/api2/json/version").await?;
        Ok(body["data"]["version"]
            .as_str()
            .unwrap_or("unknown")
            .to_string())
    }

    async fn node_online(&self, node: &str) -> Result<bool> {
        let body = self.get(&format!("/api2/json/nodes/{node}/status")).await?;
        Ok(body["data"]["status"].as_str() == Some("online"))
    }

    async fn has_storage(&self, node: &str, storage: &str) -> Result<bool> {
        let body = self
            .get(&format!("/api2/json/nodes/{node}/storage"))
            .await?;
        Ok(body["data"]
            .as_array()
            .map(|arr| arr.iter().any(|s| s["storage"].as_str() == Some(storage)))
            .unwrap_or(false))
    }

    async fn has_bridge(&self, node: &str, iface: &str) -> Result<bool> {
        let body = self
            .get(&format!("/api2/json/nodes/{node}/network"))
            .await?;
        Ok(body["data"]
            .as_array()
            .map(|arr| arr.iter().any(|s| s["iface"].as_str() == Some(iface)))
            .unwrap_or(false))
    }

    async fn vmid_in_use(&self, vmid: u32) -> Result<bool> {
        let body = self.get("/api2/json/cluster/resources").await?;
        Ok(body["data"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .any(|row| row["vmid"].as_u64() == Some(u64::from(vmid)))
            })
            .unwrap_or(false))
    }

    async fn create_lxc(&self, node: &str, body: Value) -> Result<String> {
        let res = self
            .post(&format!("/api2/json/nodes/{node}/lxc"), body)
            .await?;
        Ok(res["data"].as_str().unwrap_or("queued").to_string())
    }

    async fn create_qemu(&self, node: &str, body: Value) -> Result<String> {
        let res = self
            .post(&format!("/api2/json/nodes/{node}/qemu"), body)
            .await?;
        Ok(res["data"].as_str().unwrap_or("queued").to_string())
    }
}

/// HTTP adapter that talks to the in-process fake (or any URL) using the same paths.
pub struct HttpProxmox {
    inner: ProxmoxClient,
}

impl HttpProxmox {
    pub fn new(base: &str) -> Result<Self> {
        Ok(Self {
            inner: ProxmoxClient::new(base, "fake@pve!token", "fake", true)?,
        })
    }
}

#[async_trait]
impl ProxmoxView for HttpProxmox {
    async fn version(&self) -> Result<String> {
        self.inner.version().await
    }
    async fn node_online(&self, node: &str) -> Result<bool> {
        self.inner.node_online(node).await
    }
    async fn has_storage(&self, node: &str, storage: &str) -> Result<bool> {
        self.inner.has_storage(node, storage).await
    }
    async fn has_bridge(&self, node: &str, iface: &str) -> Result<bool> {
        self.inner.has_bridge(node, iface).await
    }
    async fn vmid_in_use(&self, vmid: u32) -> Result<bool> {
        self.inner.vmid_in_use(vmid).await
    }
    async fn create_lxc(&self, node: &str, body: Value) -> Result<String> {
        self.inner.create_lxc(node, body).await
    }
    async fn create_qemu(&self, node: &str, body: Value) -> Result<String> {
        self.inner.create_qemu(node, body).await
    }
}
