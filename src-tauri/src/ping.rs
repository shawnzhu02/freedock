//! Ping: one ICMP echo to an IPv4 address through Windows' own ICMP API (no admin needed, no
//! process spawned). IPv4 literals only, so a ping never does a DNS lookup.

use std::net::Ipv4Addr;

/// The address to ping, if `s` is an IPv4 literal like "1.1.1.1".
pub fn parse_target(s: &str) -> Option<Ipv4Addr> {
    s.trim()
        .parse::<Ipv4Addr>()
        .ok()
        .filter(|a| !a.is_unspecified() && !a.is_broadcast() && !a.is_multicast())
}

/// Round trip in ms, or None when no reply came within `timeout_ms` (counted as loss).
#[cfg(windows)]
pub fn once(target: &str, timeout_ms: u32) -> Result<Option<u32>, String> {
    use windows::Win32::NetworkManagement::IpHelper::{
        ICMP_ECHO_REPLY, IcmpCloseHandle, IcmpCreateFile, IcmpSendEcho,
    };
    let addr = parse_target(target).ok_or("Use an IPv4 address, like 1.1.1.1")?;
    let payload = [0x61u8; 32];
    // Room for one reply, its echoed payload and the 8 bytes of an ICMP error, as the docs ask.
    let mut reply = vec![0u8; std::mem::size_of::<ICMP_ECHO_REPLY>() + payload.len() + 8];
    unsafe {
        let h = IcmpCreateFile().map_err(|e| e.to_string())?;
        let n = IcmpSendEcho(
            h,
            u32::from_ne_bytes(addr.octets()),
            payload.as_ptr() as *const _,
            payload.len() as u16,
            None,
            reply.as_mut_ptr() as *mut _,
            reply.len() as u32,
            timeout_ms,
        );
        let _ = IcmpCloseHandle(h);
        if n == 0 {
            return Ok(None);
        }
        // The buffer is u8-aligned; read the reply without assuming alignment.
        let r: ICMP_ECHO_REPLY = std::ptr::read_unaligned(reply.as_ptr() as *const ICMP_ECHO_REPLY);
        Ok((r.Status == 0).then_some(r.RoundTripTime))
    }
}

#[cfg(not(windows))]
pub fn once(_target: &str, _timeout_ms: u32) -> Result<Option<u32>, String> {
    Err("Windows only".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_plain_ipv4_targets() {
        assert_eq!(parse_target(" 1.1.1.1 "), Some(Ipv4Addr::new(1, 1, 1, 1)));
        assert_eq!(parse_target("8.8.8.8"), Some(Ipv4Addr::new(8, 8, 8, 8)));
        assert_eq!(parse_target("example.com"), None);
        assert_eq!(parse_target("0.0.0.0"), None);
        assert_eq!(parse_target("255.255.255.255"), None);
        assert_eq!(parse_target("224.0.0.1"), None);
        assert_eq!(parse_target("1.1.1"), None);
    }

    #[test]
    #[ignore] // needs the network: cargo test ping -- --ignored
    fn pings_loopback() {
        assert!(once("127.0.0.1", 1000).unwrap().is_some());
    }
}
