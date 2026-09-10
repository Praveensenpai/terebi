use std::net::{IpAddr, SocketAddr, TcpStream, UdpSocket};
use std::process::Command;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

#[must_use]
pub fn find_attached_adb_device() -> Option<String> {
    let output = Command::new("adb").args(["devices"]).output().ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    for line in text.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 && parts[1] == "device" && parts[0].contains(':') {
            if let Some(ip) = parts[0].split(':').next() {
                return Some(ip.to_string());
            }
        }
    }
    None
}

#[must_use]
pub fn detect_local_subnet_prefix() -> Option<String> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    match socket.local_addr().ok()?.ip() {
        IpAddr::V4(ipv4) => {
            let oct = ipv4.octets();
            Some(format!("{}.{}.{}.", oct[0], oct[1], oct[2]))
        }
        IpAddr::V6(_) => None,
    }
}

#[must_use]
pub fn scan_subnet_for_adb(port: u16) -> Option<String> {
    let prefix = detect_local_subnet_prefix()?;
    let (tx, rx) = mpsc::channel();
    let mut handles = Vec::with_capacity(254);

    for i in 1..=254 {
        let tx = tx.clone();
        let target_str = format!("{prefix}{i}:{port}");
        let Ok(addr) = target_str.parse::<SocketAddr>() else {
            continue;
        };

        let handle = thread::spawn(move || {
            if TcpStream::connect_timeout(&addr, Duration::from_millis(250)).is_ok() {
                let _ = tx.send(addr.ip().to_string());
            }
        });
        handles.push(handle);
    }
    drop(tx);

    if let Ok(found_ip) = rx.recv_timeout(Duration::from_millis(1000)) {
        return Some(found_ip);
    }
    None
}

#[must_use]
pub fn discover_tv_ip(port: u16) -> Option<String> {
    if let Some(ip) = find_attached_adb_device() {
        return Some(ip);
    }
    scan_subnet_for_adb(port)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_local_subnet() {
        let prefix = detect_local_subnet_prefix();
        assert!(prefix.is_some());
    }
}
