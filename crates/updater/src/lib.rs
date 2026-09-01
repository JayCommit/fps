//! Update channel policy and signed `update-manifest.json` verification.
//!
//! GitHub `/releases/latest` is never used for alpha or beta channels because
//! GitHub excludes prereleases from that endpoint. See [`check`].

pub mod check;

pub use check::{github_releases_api_path, github_releases_url, release_list_url};

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use fps_branding::Channel;
use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum UpdateError {
    #[error("manifest schema is unsupported: {0}")]
    UnsupportedSchema(u32),
    #[error("manifest signature is invalid")]
    BadSignature,
    #[error("artifact digest mismatch")]
    BadDigest,
    #[error("channel {channel} cannot install {version}")]
    ChannelDenied { channel: String, version: String },
    #[error("no eligible release for channel {0}")]
    NoEligibleRelease(String),
    #[error("{0}")]
    Other(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateManifest {
    pub schema_version: u32,
    pub product_version: String,
    pub channel: Channel,
    pub git_tag: String,
    pub git_commit: String,
    pub published_at: String,
    pub release_notes_url: String,
    pub min_control_plane: String,
    pub min_node_protocol: u16,
    pub min_desktop: String,
    pub min_bootstrap: String,
    pub min_database_schema: u32,
    pub assets: Vec<ManifestAsset>,
    pub migrations_required: bool,
    pub restart_required: bool,
    pub rollback_supported: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestAsset {
    pub name: String,
    pub url: String,
    pub size: u64,
    pub sha256: String,
    pub content_type: String,
    pub platform: String,
}

#[derive(Debug, Clone)]
pub struct PublishedRelease {
    pub tag: String,
    pub version: Version,
    pub draft: bool,
    pub prerelease: bool,
}

/// Select the highest SemVer that the channel may install.
///
/// * `alpha` — any newer version (alpha, beta, or stable)
/// * `beta` — beta and stable only (never alpha)
/// * `stable` — non-prerelease only
pub fn select_release<'a>(
    channel: Channel,
    current: &Version,
    releases: &'a [PublishedRelease],
) -> Result<&'a PublishedRelease, UpdateError> {
    let mut best: Option<&PublishedRelease> = None;
    for rel in releases {
        if rel.draft {
            continue;
        }
        if !channel_allows(channel, &rel.version) {
            continue;
        }
        if &rel.version <= current {
            continue;
        }
        if best.map(|b| rel.version > b.version).unwrap_or(true) {
            best = Some(rel);
        }
    }
    best.ok_or_else(|| UpdateError::NoEligibleRelease(channel.as_str().to_string()))
}

pub fn channel_allows(channel: Channel, version: &Version) -> bool {
    match channel {
        Channel::Alpha => true,
        Channel::Beta => version.pre.is_empty() || version.pre.as_str().starts_with("beta"),
        Channel::Stable => version.pre.is_empty(),
    }
}

pub fn verify_digest(bytes: &[u8], expected_sha256_hex: &str) -> Result<(), UpdateError> {
    let actual = hex::encode(Sha256::digest(bytes));
    if actual.eq_ignore_ascii_case(expected_sha256_hex) {
        Ok(())
    } else {
        Err(UpdateError::BadDigest)
    }
}

/// Signatures are Ed25519 over the canonical JSON bytes of the manifest
/// (no whitespace variance: `serde_json` compact serialization).
pub fn verify_manifest_signature(
    canonical_json: &[u8],
    signature: &Signature,
    public_key: &VerifyingKey,
) -> Result<(), UpdateError> {
    public_key
        .verify(canonical_json, signature)
        .map_err(|_| UpdateError::BadSignature)
}

pub fn canonical_json(manifest: &UpdateManifest) -> Result<Vec<u8>, UpdateError> {
    serde_json::to_vec(manifest).map_err(|e| UpdateError::Other(e.to_string()))
}

pub fn signing_key_from_hex(secret_hex: &str) -> Result<SigningKey, UpdateError> {
    let raw = hex::decode(secret_hex.trim()).map_err(|e| UpdateError::Other(e.to_string()))?;
    let bytes: [u8; 32] = raw.try_into().map_err(|_| {
        UpdateError::Other("signing key must be 32 bytes encoded as 64 hex characters".into())
    })?;
    Ok(SigningKey::from_bytes(&bytes))
}

pub fn sign_manifest(
    manifest: &UpdateManifest,
    signing_key: &SigningKey,
) -> Result<(Vec<u8>, String), UpdateError> {
    let canonical = canonical_json(manifest)?;
    let signature = signing_key.sign(&canonical);
    Ok((canonical, hex::encode(signature.to_bytes())))
}

pub fn parse_signature_hex(hex_sig: &str) -> Result<Signature, UpdateError> {
    let raw = hex::decode(hex_sig.trim()).map_err(|e| UpdateError::Other(e.to_string()))?;
    let bytes: [u8; 64] = raw
        .try_into()
        .map_err(|_| UpdateError::Other("signature must be 64 bytes hex".into()))?;
    Ok(Signature::from_bytes(&bytes))
}

pub fn parse_manifest(json: &[u8]) -> Result<UpdateManifest, UpdateError> {
    let manifest: UpdateManifest =
        serde_json::from_slice(json).map_err(|e| UpdateError::Other(e.to_string()))?;
    if manifest.schema_version != 1 {
        return Err(UpdateError::UnsupportedSchema(manifest.schema_version));
    }
    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    use rand::rngs::OsRng;

    fn v(s: &str) -> Version {
        s.parse().unwrap()
    }

    fn rel(tag: &str, draft: bool) -> PublishedRelease {
        PublishedRelease {
            version: v(tag.trim_start_matches('v')),
            tag: tag.to_string(),
            draft,
            prerelease: !v(tag.trim_start_matches('v')).pre.is_empty(),
        }
    }

    #[test]
    fn stable_skips_alpha_and_drafts() {
        let releases = vec![
            rel("v0.0.1-alpha.2", false),
            rel("v0.0.1-beta.1", false),
            rel("v0.0.1", true),
        ];
        let err = select_release(Channel::Stable, &v("0.0.1-alpha.1"), &releases).unwrap_err();
        assert!(matches!(err, UpdateError::NoEligibleRelease(_)));
    }

    #[test]
    fn beta_skips_alpha_but_takes_newer_beta() {
        let releases = vec![
            rel("v0.0.1-alpha.9", false),
            rel("v0.0.1-beta.2", false),
            rel("v0.0.1-beta.1", false),
        ];
        let chosen = select_release(Channel::Beta, &v("0.0.1-beta.1"), &releases).unwrap();
        assert_eq!(chosen.tag, "v0.0.1-beta.2");
    }

    #[test]
    fn alpha_may_install_stable() {
        let releases = vec![rel("v0.0.1", false), rel("v0.0.1-alpha.2", false)];
        let chosen = select_release(Channel::Alpha, &v("0.0.1-alpha.1"), &releases).unwrap();
        assert_eq!(chosen.tag, "v0.0.1");
    }

    #[test]
    fn signed_manifest_round_trip() {
        let mut rng = OsRng;
        let signing = SigningKey::generate(&mut rng);
        let manifest = UpdateManifest {
            schema_version: 1,
            product_version: "0.0.1-alpha.1".into(),
            channel: Channel::Alpha,
            git_tag: "v0.0.1-alpha.1".into(),
            git_commit: "deadbeef".into(),
            published_at: "2026-09-01T00:00:00Z".into(),
            release_notes_url: "https://example.test/notes".into(),
            min_control_plane: "0.0.1-alpha.1".into(),
            min_node_protocol: 1,
            min_desktop: "0.0.1-alpha.5".into(),
            min_bootstrap: "0.0.1-alpha.1".into(),
            min_database_schema: 1,
            assets: vec![ManifestAsset {
                name: "fps-control-plane-x86_64-linux".into(),
                url: "https://example.test/cp".into(),
                size: 12,
                sha256: hex::encode(Sha256::digest(b"hello-release")),
                content_type: "application/octet-stream".into(),
                platform: "linux-x86_64".into(),
            }],
            migrations_required: true,
            restart_required: true,
            rollback_supported: false,
        };
        let canonical = canonical_json(&manifest).unwrap();
        let sig = signing.sign(&canonical);
        verify_manifest_signature(&canonical, &sig, &signing.verifying_key()).unwrap();
        let (again, hex_sig) = sign_manifest(&manifest, &signing).unwrap();
        assert_eq!(again, canonical);
        verify_manifest_signature(
            &canonical,
            &parse_signature_hex(&hex_sig).unwrap(),
            &signing.verifying_key(),
        )
        .unwrap();
        verify_digest(b"hello-release", &manifest.assets[0].sha256).unwrap();
        assert!(verify_digest(b"tampered", &manifest.assets[0].sha256).is_err());
    }
}
