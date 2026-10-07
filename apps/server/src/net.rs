//! Where the server listens.

use std::io;
use std::net::{Ipv4Addr, Ipv6Addr, TcpListener};

/// The sockets to serve on. A `host` binds exactly there. Without one, every
/// interface on IPv6 and IPv4, so `localhost` works whichever of `::1` and
/// `127.0.0.1` a client resolves it to; a machine (or container) without
/// IPv6 falls back to IPv4 alone.
pub fn listeners(host: Option<&str>, port: u16) -> io::Result<Vec<TcpListener>> {
    if let Some(host) = host {
        return Ok(vec![TcpListener::bind((host, port))?]);
    }
    let Ok(v6) = TcpListener::bind((Ipv6Addr::UNSPECIFIED, port)) else {
        return Ok(vec![TcpListener::bind((Ipv4Addr::UNSPECIFIED, port))?]);
    };
    // Usually `::` takes IPv4 too and this bind is refused; where IPv6
    // sockets are IPv6-only it succeeds and serves IPv4 itself.
    let port = v6.local_addr()?.port();
    let mut sockets = vec![v6];
    sockets.extend(TcpListener::bind((Ipv4Addr::UNSPECIFIED, port)).ok());
    Ok(sockets)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpStream;

    #[test]
    fn without_a_host_both_loopbacks_reach_the_server() {
        let sockets = listeners(None, 0).unwrap();
        let port = sockets[0].local_addr().unwrap().port();
        assert!(TcpStream::connect((Ipv4Addr::LOCALHOST, port)).is_ok());
        if sockets[0].local_addr().unwrap().is_ipv6() {
            assert!(TcpStream::connect((Ipv6Addr::LOCALHOST, port)).is_ok());
        }
    }

    #[test]
    fn a_host_is_bound_exactly() {
        let sockets = listeners(Some("127.0.0.1"), 0).unwrap();
        assert_eq!(sockets.len(), 1);
        assert_eq!(
            sockets[0].local_addr().unwrap().ip(),
            std::net::IpAddr::V4(Ipv4Addr::LOCALHOST)
        );
    }
}
