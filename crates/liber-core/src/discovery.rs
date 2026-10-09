use std::time::Duration;

use crate::CoreError;

pub const SERVICE_TYPE: &str = "_liber._tcp.local.";
pub const SERVICE_VERSION: &str = "2";

pub fn instance_name(device_id: &str) -> String {
    format!("liber-{device_id}")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Peer {
    pub instance: String,
    pub host: String,
    pub port: u16,
    pub device_id: String,
}

impl Peer {
    pub fn url(&self) -> String {
        format!("http://{}:{}", self.host, self.port)
    }
}

pub fn qr_ascii(url: &str) -> Result<String, CoreError> {
    let code = qrcode::QrCode::new(url.as_bytes())
        .map_err(|e| CoreError::Invalid(format!("QR payload rejected: {e}")))?;
    Ok(code.render::<char>().quiet_zone(false).build())
}

pub fn lan_ip() -> Option<std::net::IpAddr> {
    let sock = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    sock.connect("8.8.8.8:80").ok()?;
    let ip = sock.local_addr().ok()?.ip();
    if ip.is_loopback() {
        return None;
    }
    Some(ip)
}

pub struct Advertiser {
    _daemon: mdns_sd::ServiceDaemon,
}

pub fn advertise(port: u16, device_id: &str) -> Result<Option<Advertiser>, CoreError> {
    let Some(ip) = lan_ip() else {
        return Ok(None);
    };
    let instance = instance_name(device_id);
    let host = format!("{instance}.local.");
    let props = [("device", device_id), ("version", SERVICE_VERSION)];
    let info = mdns_sd::ServiceInfo::new(
        SERVICE_TYPE,
        &instance,
        &host,
        ip.to_string(),
        port,
        &props[..],
    )
    .map_err(|e| CoreError::Storage(format!("mdns service info: {e}")))?;
    let daemon =
        mdns_sd::ServiceDaemon::new().map_err(|e| CoreError::Storage(format!("mdns: {e}")))?;
    daemon
        .register(info)
        .map_err(|e| CoreError::Storage(format!("mdns register: {e}")))?;
    Ok(Some(Advertiser { _daemon: daemon }))
}

fn peer_of(info: &mdns_sd::ServiceInfo) -> Peer {
    let device_id = info
        .get_property_val_str("device")
        .unwrap_or_default()
        .to_string();
    let (host, port) = match (info.get_addresses().iter().next(), info.get_port()) {
        (Some(ip), port) => (ip.to_string(), port),
        (None, port) => (info.get_hostname().trim_end_matches('.').to_string(), port),
    };
    Peer {
        instance: info.get_fullname().to_string(),
        host,
        port,
        device_id,
    }
}

pub fn browse(timeout: Duration) -> Result<Vec<Peer>, CoreError> {
    let daemon =
        mdns_sd::ServiceDaemon::new().map_err(|e| CoreError::Storage(format!("mdns: {e}")))?;
    let receiver = daemon
        .browse(SERVICE_TYPE)
        .map_err(|e| CoreError::Storage(format!("mdns browse: {e}")))?;
    let deadline = std::time::Instant::now() + timeout;
    let mut peers = Vec::new();
    loop {
        let now = std::time::Instant::now();
        if now >= deadline {
            break;
        }
        match receiver.recv_timeout(deadline - now) {
            Ok(mdns_sd::ServiceEvent::ServiceResolved(info)) => {
                let peer = peer_of(&info);
                if !peers.iter().any(|p: &Peer| p.instance == peer.instance) {
                    peers.push(peer);
                }
            }
            Ok(_) => {}
            Err(_) => break,
        }
    }
    daemon.shutdown().ok();
    peers.sort_by(|a, b| a.instance.cmp(&b.instance));
    Ok(peers)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qr_renders_dense_blocks() {
        let art = qr_ascii("http://192.168.1.10:8080").unwrap();
        assert!(art.lines().count() >= 10);
        assert!(art.contains('\u{2588}') || art.contains('\u{2580}') || art.contains('\u{2584}'));
        let again = qr_ascii("http://192.168.1.10:8080").unwrap();
        assert_eq!(art, again);
    }

    #[test]
    fn peer_url_shape() {
        let peer = Peer {
            instance: "liber-abc._liber._tcp.local.".to_string(),
            host: "192.168.1.10".to_string(),
            port: 8080,
            device_id: "abc".to_string(),
        };
        assert_eq!(peer.url(), "http://192.168.1.10:8080");
    }

    #[test]
    fn instance_naming() {
        assert_eq!(instance_name("abc"), "liber-abc");
        assert!(SERVICE_TYPE.ends_with(".local."));
    }

    #[test]
    fn browse_zero_timeout_returns_empty() {
        let peers = browse(Duration::from_secs(0)).unwrap();
        assert!(peers.is_empty());
    }
}
