//! Refuse local-network addresses where it can't be raced: in the resolver the HTTP client connects
//! through, so a redirect, or a name that resolves to a private address on the second lookup, is refused too.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

use ureq::config::Config;
use ureq::http::Uri;
use ureq::unversioned::resolver::{DefaultResolver, ResolvedSocketAddrs, Resolver};
use ureq::unversioned::transport::NextTimeout;

/// Whether an address is on the public internet. Everything else (this computer, the home or office
/// network, link-local, carrier NAT, multicast, documentation and reserved ranges) is refused.
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
    let s = ip.segments();
    // NAT64 and 6to4 carry an IPv4 address inside: judge that one.
    if s[0] == 0x64 && s[1] == 0xff9b && s[2..6] == [0; 4] {
        return is_public_v4(Ipv4Addr::new(
            (s[6] >> 8) as u8,
            s[6] as u8,
            (s[7] >> 8) as u8,
            s[7] as u8,
        ));
    }
    if s[0] == 0x2002 {
        return is_public_v4(Ipv4Addr::new(
            (s[1] >> 8) as u8,
            s[1] as u8,
            (s[2] >> 8) as u8,
            s[2] as u8,
        ));
    }
    // Only global unicast (2000::/3), minus Teredo (2001::/32) and documentation (2001:db8::/32).
    (s[0] & 0xe000) == 0x2000 && !(s[0] == 0x2001 && (s[1] == 0 || s[1] == 0xdb8))
}

/// The resolver: looks up as usual, then keeps only public addresses (or the one test server).
#[derive(Debug, Default)]
pub struct PublicOnly {
    /// Tests only: the local test server, the one non-public address allowed.
    pub allow: Option<SocketAddr>,
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
