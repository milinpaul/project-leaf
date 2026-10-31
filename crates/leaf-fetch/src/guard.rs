//! Refusing local-network addresses where it can't be raced: in the DNS lookup the connection itself
//! uses (ADR-080). Checking an address before the request would leave two gaps: a redirect to another
//! host, and a name that answers with a public address first and a private one the second time. Here
//! every hop's lookup is filtered, so neither gets through.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

use ureq::config::Config;
use ureq::http::Uri;
use ureq::unversioned::resolver::{DefaultResolver, ResolvedSocketAddrs, Resolver};
use ureq::unversioned::transport::NextTimeout;

/// Whether an address is on the public internet. Everything else is refused: this computer, the home
/// or office network, link-local (the cloud metadata address among them), carrier NAT, multicast,
/// documentation and reserved ranges, and IPv4 addresses hidden in IPv6 that point at any of those.
#[must_use]
pub fn is_public(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_public_v4(v4),
        IpAddr::V6(v6) => is_public_v6(v6),
    }
}

fn is_public_v4(ip: Ipv4Addr) -> bool {
    let [a, b, c, _] = ip.octets();
    !(a == 0
        || a == 10
        || a == 127
        || (a == 100 && (64..128).contains(&b))
        || (a == 169 && b == 254)
        || (a == 172 && (16..32).contains(&b))
        || (a == 192 && b == 168)
        || (a == 192 && b == 0 && (c == 0 || c == 2))
        || (a == 198 && (b == 18 || b == 19))
        || (a == 198 && b == 51 && c == 100)
        || (a == 203 && b == 0 && c == 113)
        || a >= 224)
}

fn is_public_v6(ip: Ipv6Addr) -> bool {
    if let Some(v4) = ip.to_ipv4_mapped() {
        return is_public_v4(v4);
    }
    let o = ip.octets();
    let v4 = |at: usize| Ipv4Addr::new(o[at], o[at + 1], o[at + 2], o[at + 3]);
    // NAT64 (64:ff9b::/96) and 6to4 (2002::/16) carry an IPv4 address inside: judge that one.
    if o[..12] == [0, 0x64, 0xff, 0x9b, 0, 0, 0, 0, 0, 0, 0, 0] {
        return is_public_v4(v4(12));
    }
    if o[..2] == [0x20, 0x02] {
        return is_public_v4(v4(2));
    }
    // Only global unicast (2000::/3), minus Teredo (2001::/32) and documentation (2001:db8::/32).
    let teredo = o[..4] == [0x20, 0x01, 0, 0];
    let documentation = o[..4] == [0x20, 0x01, 0x0d, 0xb8];
    (o[0] & 0xe0) == 0x20 && !teredo && !documentation
}

/// The resolver every fetch connects through: looks names up as usual, then keeps only public addresses.
#[derive(Debug, Default)]
pub struct PublicOnly {
    /// The one other address allowed: a local test server.
    allow: Option<SocketAddr>,
}

impl PublicOnly {
    /// Also allows `server`, a test server on this computer. Only tests use it: the app never does.
    #[must_use]
    pub fn allowing(server: SocketAddr) -> Self {
        Self { allow: Some(server) }
    }
}

impl Resolver for PublicOnly {
    fn resolve(
        &self,
        uri: &Uri,
        config: &Config,
        timeout: NextTimeout,
    ) -> Result<ResolvedSocketAddrs, ureq::Error> {
        let found = DefaultResolver::default().resolve(uri, config, timeout)?;
        let mut kept = self.empty();
        for &addr in &*found {
            if is_public(addr.ip()) || Some(addr) == self.allow {
                kept.push(addr);
            }
        }
        if kept.is_empty() {
            Err(ureq::Error::HostNotFound)
        } else {
            Ok(kept)
        }
    }
}
