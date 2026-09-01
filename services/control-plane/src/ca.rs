use std::fs;
use std::net::{IpAddr, SocketAddr};
use std::path::Path;

use rcgen::{
    BasicConstraints, CertificateParams, DistinguishedName, DnType, ExtendedKeyUsagePurpose, IsCa,
    Issuer, KeyPair, KeyUsagePurpose, SanType,
};
use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CaError {
    #[error("certificate authority error: {0}")]
    Rcgen(#[from] rcgen::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

pub struct IssuedCert {
    pub certificate_pem: String,
    pub private_key_pem: String,
    pub ca_pem: String,
    pub fingerprint_sha256: String,
}

pub struct CertificateAuthority {
    ca_pem: String,
    key_pem: String,
}

impl CertificateAuthority {
    pub fn load_or_create(dir: &Path) -> Result<Self, CaError> {
        fs::create_dir_all(dir)?;
        let cert_path = dir.join("ca.pem");
        let key_path = dir.join("ca.key");
        if cert_path.exists() && key_path.exists() {
            return Ok(Self {
                ca_pem: fs::read_to_string(&cert_path)?,
                key_pem: fs::read_to_string(&key_path)?,
            });
        }
        let issuer_key = KeyPair::generate()?;
        let mut params = CertificateParams::default();
        params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        params.key_usages = vec![
            KeyUsagePurpose::KeyCertSign,
            KeyUsagePurpose::CrlSign,
            KeyUsagePurpose::DigitalSignature,
        ];
        let mut dn = DistinguishedName::new();
        dn.push(
            DnType::CommonName,
            format!("{} Node CA", fps_branding::DISPLAY_NAME),
        );
        params.distinguished_name = dn;
        let cert = params.self_signed(&issuer_key)?;
        let ca_pem = cert.pem();
        let key_pem = issuer_key.serialize_pem();
        fs::write(&cert_path, &ca_pem)?;
        fs::write(&key_path, &key_pem)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&key_path)?.permissions();
            perms.set_mode(0o600);
            fs::set_permissions(&key_path, perms)?;
        }
        Ok(Self { ca_pem, key_pem })
    }

    pub fn ca_pem(&self) -> &str {
        &self.ca_pem
    }

    fn issuer(&self) -> Result<Issuer<'_, KeyPair>, CaError> {
        let issuer_key = KeyPair::from_pem(&self.key_pem)?;
        let mut issuer_params = CertificateParams::default();
        issuer_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        issuer_params.key_usages = vec![
            KeyUsagePurpose::KeyCertSign,
            KeyUsagePurpose::CrlSign,
            KeyUsagePurpose::DigitalSignature,
        ];
        let mut issuer_dn = DistinguishedName::new();
        issuer_dn.push(
            DnType::CommonName,
            format!("{} Node CA", fps_branding::DISPLAY_NAME),
        );
        issuer_params.distinguished_name = issuer_dn;
        Ok(Issuer::new(issuer_params, issuer_key))
    }

    pub fn issue_node_cert(&self, node_id: &str, hostname: &str) -> Result<IssuedCert, CaError> {
        let issuer = self.issuer()?;
        let mut params = CertificateParams::new(vec![hostname.to_string(), node_id.to_string()])?;
        params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ClientAuth];
        params.key_usages = vec![KeyUsagePurpose::DigitalSignature];
        let mut dn = DistinguishedName::new();
        dn.push(DnType::CommonName, format!("node:{node_id}"));
        params.distinguished_name = dn;
        params.subject_alt_names.push(SanType::DnsName(
            hostname
                .try_into()
                .map_err(|_| rcgen::Error::CouldNotParseCertificate)?,
        ));
        let key = KeyPair::generate()?;
        let cert = params.signed_by(&key, &issuer)?;
        let certificate_pem = cert.pem();
        let der = cert.der();
        let fingerprint_sha256 = hex::encode(Sha256::digest(der.as_ref()));
        Ok(IssuedCert {
            certificate_pem,
            private_key_pem: key.serialize_pem(),
            ca_pem: self.ca_pem.clone(),
            fingerprint_sha256,
        })
    }

    /// Server certificate for the node mTLS listener.
    pub fn issue_server_cert(
        &self,
        bind: SocketAddr,
        public_url: &str,
    ) -> Result<(String, String), CaError> {
        let issuer = self.issuer()?;
        let mut params = CertificateParams::new(vec!["control-plane".into()])?;
        params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
        params.key_usages = vec![KeyUsagePurpose::DigitalSignature];
        let mut dn = DistinguishedName::new();
        dn.push(
            DnType::CommonName,
            format!("{} node endpoint", fps_branding::DISPLAY_NAME),
        );
        params.distinguished_name = dn;
        params.subject_alt_names.push(SanType::DnsName(
            "localhost"
                .try_into()
                .map_err(|_| rcgen::Error::CouldNotParseCertificate)?,
        ));
        let mut ips = vec![IpAddr::from([127, 0, 0, 1])];
        if !bind.ip().is_unspecified() && !ips.contains(&bind.ip()) {
            ips.push(bind.ip());
        }
        if let Ok(url) = url::Url::parse(public_url) {
            if let Some(host) = url.host_str() {
                if let Ok(ip) = host.parse::<IpAddr>() {
                    if !ips.contains(&ip) {
                        ips.push(ip);
                    }
                } else if host != "localhost" {
                    if let Ok(dns) = host.try_into() {
                        params.subject_alt_names.push(SanType::DnsName(dns));
                    }
                }
            }
        }
        for ip in ips {
            params.subject_alt_names.push(SanType::IpAddress(ip));
        }
        let key = KeyPair::generate()?;
        let cert = params.signed_by(&key, &issuer)?;
        Ok((cert.pem(), key.serialize_pem()))
    }
}
