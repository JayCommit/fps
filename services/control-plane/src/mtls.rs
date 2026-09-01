use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::{Context, Result};
use axum::body::Body;
use axum::Router;
use hyper::body::Incoming;
use hyper::Request;
use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto::Builder as HyperBuilder;
use rustls::pki_types::CertificateDer;
use rustls::server::WebPkiClientVerifier;
use rustls::{RootCertStore, ServerConfig};
use sha2::{Digest, Sha256};
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;
use tower::ServiceExt;
use tracing::{debug, warn};

use crate::ca::CertificateAuthority;
use crate::http::extractors::PeerFingerprint;

pub fn install_crypto_provider() {
    let _ = rustls::crypto::ring::default_provider().install_default();
}

pub fn server_config(
    ca: &CertificateAuthority,
    bind: SocketAddr,
    public_url: &str,
) -> Result<Arc<ServerConfig>> {
    install_crypto_provider();
    let (cert_pem, key_pem) = ca
        .issue_server_cert(bind, public_url)
        .context("issue node listener certificate")?;
    let certs = rustls_pemfile::certs(&mut cert_pem.as_bytes())
        .collect::<Result<Vec<_>, _>>()
        .context("parse server cert")?;
    let key = rustls_pemfile::private_key(&mut key_pem.as_bytes())
        .context("parse server key")?
        .context("server key missing")?;

    let mut roots = RootCertStore::empty();
    for cert in rustls_pemfile::certs(&mut ca.ca_pem().as_bytes()) {
        roots
            .add(cert.context("parse CA cert")?)
            .context("add CA to trust store")?;
    }
    let verifier = WebPkiClientVerifier::builder(Arc::new(roots))
        .build()
        .map_err(|e| anyhow::anyhow!("client verifier: {e}"))?;
    let mut config = ServerConfig::builder()
        .with_client_cert_verifier(verifier)
        .with_single_cert(certs, key)
        .context("rustls server config")?;
    config.alpn_protocols = vec![b"http/1.1".to_vec()];
    Ok(Arc::new(config))
}

pub async fn accept_loop(listener: TcpListener, acceptor: TlsAcceptor, app: Router) -> Result<()> {
    loop {
        let (stream, addr) = listener.accept().await.context("mtls accept")?;
        let acceptor = acceptor.clone();
        let app = app.clone();
        tokio::spawn(async move {
            if let Err(err) = handle_conn(acceptor, stream, addr, app).await {
                debug!(%addr, error = %err, "mtls connection ended");
            }
        });
    }
}

async fn handle_conn(
    acceptor: TlsAcceptor,
    stream: tokio::net::TcpStream,
    addr: SocketAddr,
    app: Router,
) -> Result<()> {
    let tls = acceptor.accept(stream).await.with_context(|| {
        warn!(%addr, "mtls handshake failed");
        format!("handshake from {addr}")
    })?;
    let fp = tls
        .get_ref()
        .1
        .peer_certificates()
        .and_then(|certs: &[CertificateDer<'_>]| certs.first())
        .map(|c| hex::encode(Sha256::digest(c.as_ref())))
        .unwrap_or_default();
    let io = TokioIo::new(tls);
    let service = hyper::service::service_fn(move |req: Request<Incoming>| {
        let app = app.clone();
        let fp = fp.clone();
        async move {
            let mut req = req.map(Body::new);
            req.extensions_mut().insert(PeerFingerprint(fp));
            app.oneshot(req).await
        }
    });
    HyperBuilder::new(TokioExecutor::new())
        .serve_connection(io, service)
        .await
        .map_err(|e| anyhow::anyhow!("mtls http: {e}"))?;
    Ok(())
}
