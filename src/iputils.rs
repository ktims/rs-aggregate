#[cfg(feature = "rayon")]
use rayon::join;
use std::{
    error::Error,
    fmt::Display,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    str::FromStr,
};

use ipnet::{IpNet, Ipv4Net, Ipv6Net};

#[derive(Default)]
pub struct IpBothRange {
    v4: Vec<Ipv4Net>,
    v6: Vec<Ipv6Net>,
}

impl IpBothRange {
    pub fn new() -> IpBothRange {
        IpBothRange::default()
    }
    pub fn add(&mut self, net: IpOrNet) {
        match net.0 {
            IpNet::V4(n) => self.v4.push(n),
            IpNet::V6(n) => self.v6.push(n),
        }
    }
    #[cfg(feature = "rayon")]
    pub fn simplify(&mut self) {
        (self.v4, self.v6) = join(
            || Ipv4Net::aggregate(&self.v4),
            || Ipv6Net::aggregate(&self.v6),
        );
    }
    #[cfg(not(feature = "rayon"))]
    pub fn simplify(&mut self) {
        self.v4 = Ipv4Net::aggregate(&self.v4);
        self.v6 = Ipv6Net::aggregate(&self.v6);
    }

    pub fn v4_iter(&self) -> impl Iterator<Item = &Ipv4Net> {
        self.v4.iter()
    }

    pub fn v6_iter(&self) -> impl Iterator<Item = &Ipv6Net> {
        self.v6.iter()
    }
}

impl Display for IpBothRange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for ip in self {
            ip.fmt(f)?;
            writeln!(f)?;
        }
        Ok(())
    }
}

pub struct IpBothRangeIter<'a> {
    ranges: &'a IpBothRange,
    v4done: bool,
    pos: usize,
}

impl<'a> Iterator for IpBothRangeIter<'a> {
    type Item = IpNet;
    fn next(&mut self) -> Option<Self::Item> {
        if !self.v4done {
            if self.pos < self.ranges.v4.len() {
                self.pos += 1;
                Some(IpNet::V4(self.ranges.v4[self.pos - 1]))
            } else {
                self.v4done = true;
                self.pos = 0;
                self.next()
            }
        } else if self.pos < self.ranges.v6.len() {
            self.pos += 1;
            Some(IpNet::V6(self.ranges.v6[self.pos - 1]))
        } else {
            None
        }
    }
}

impl<'a> IntoIterator for &'a IpBothRange {
    type Item = IpNet;
    type IntoIter = IpBothRangeIter<'a>;
    fn into_iter(self) -> Self::IntoIter {
        IpBothRangeIter {
            ranges: self,
            v4done: false,
            pos: 0,
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct IpOrNet(IpNet);

#[derive(Debug, Clone)]
pub struct NetParseError {
    #[allow(dead_code)]
    msg: &'static str,
}

impl Display for NetParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Unable to parse address")
    }
}

impl Error for NetParseError {}

impl IpOrNet {
    // Accepted formats:
    //   netmask - 1.1.1.0/255.255.255.0
    //   wildcard mask - 1.1.1.0/0.0.0.255
    fn parse_mask(p: &str) -> Result<u8, Box<dyn Error>> {
        let mask = p.parse::<Ipv4Addr>()?;
        let intrep: u32 = mask.into();
        let lead_ones = intrep.leading_ones();
        if lead_ones > 0 {
            if lead_ones + intrep.trailing_zeros() == 32 {
                Ok(lead_ones.try_into()?)
            } else {
                Err(Box::new(NetParseError {
                    msg: "Invalid subnet mask",
                }))
            }
        } else {
            let lead_zeros = intrep.leading_zeros();
            if lead_zeros + intrep.trailing_ones() == 32 {
                Ok(lead_zeros.try_into()?)
            } else {
                Err(Box::new(NetParseError {
                    msg: "Invalid wildcard mask",
                }))
            }
        }
    }

    fn from_parts(ip: &str, pfxlen: &str) -> Result<Self, Box<dyn Error>> {
        let ip = ip.parse::<IpAddr>()?;
        let pfxlenp = pfxlen.parse::<u8>();

        match pfxlenp {
            Ok(pfxlen) => Ok(IpNet::new(ip, pfxlen)?.into()),
            Err(_) => {
                if ip.is_ipv4() {
                    Ok(IpNet::new(ip, IpOrNet::parse_mask(pfxlen)?)?.into())
                } else {
                    Err(Box::new(NetParseError {
                        msg: "Mask form is not valid for IPv6 address",
                    }))
                }
            }
        }
    }
    pub fn prefix_len(&self) -> u8 {
        self.0.prefix_len()
    }
    pub fn is_ipv4(&self) -> bool {
        match self.0 {
            IpNet::V4(_) => true,
            IpNet::V6(_) => false,
        }
    }
    pub fn is_ipv6(&self) -> bool {
        match self.0 {
            IpNet::V4(_) => false,
            IpNet::V6(_) => true,
        }
    }
    pub fn addr(&self) -> IpAddr {
        self.0.addr()
    }
    pub fn network(&self) -> IpAddr {
        self.0.network()
    }
    pub fn has_host_bits(&self) -> bool {
        self.0.addr() != self.0.network()
    }
}

impl FromStr for IpOrNet {
    type Err = Box<dyn Error>;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts = s.split_once('/');
        match parts {
            Some((ip, pfxlen)) => IpOrNet::from_parts(ip, pfxlen),
            None => Ok(s.parse::<IpAddr>()?.into()),
        }
    }
}

impl Display for IpOrNet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl From<IpNet> for IpOrNet {
    fn from(net: IpNet) -> Self {
        IpOrNet(net)
    }
}

impl From<IpAddr> for IpOrNet {
    fn from(addr: IpAddr) -> Self {
        IpOrNet(addr.into())
    }
}

impl From<Ipv4Net> for IpOrNet {
    fn from(net: Ipv4Net) -> Self {
        IpOrNet(net.into())
    }
}

impl From<Ipv6Net> for IpOrNet {
    fn from(net: Ipv6Net) -> Self {
        IpOrNet(net.into())
    }
}

impl From<Ipv4Addr> for IpOrNet {
    fn from(addr: Ipv4Addr) -> Self {
        IpOrNet(IpAddr::from(addr).into())
    }
}

impl From<Ipv6Addr> for IpOrNet {
    fn from(addr: Ipv6Addr) -> Self {
        IpOrNet(IpAddr::from(addr).into())
    }
}

#[derive(Clone, Debug)]
pub struct PrefixlenPair {
    pub v4: u8,
    pub v6: u8,
}

impl Default for PrefixlenPair {
    fn default() -> Self {
        PrefixlenPair { v4: 32, v6: 128 }
    }
}

impl Display for PrefixlenPair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{},{}", self.v4, self.v6))
    }
}

impl PartialEq<IpOrNet> for PrefixlenPair {
    fn eq(&self, other: &IpOrNet) -> bool {
        match other.0 {
            IpNet::V4(net) => self.v4 == net.prefix_len(),
            IpNet::V6(net) => self.v6 == net.prefix_len(),
        }
    }
}

impl PartialEq<PrefixlenPair> for PrefixlenPair {
    fn eq(&self, other: &PrefixlenPair) -> bool {
        self.v4 == other.v4 && self.v6 == other.v6
    }
}

impl PartialOrd<IpOrNet> for PrefixlenPair {
    fn ge(&self, other: &IpOrNet) -> bool {
        match other.0 {
            IpNet::V4(net) => self.v4 >= net.prefix_len(),
            IpNet::V6(net) => self.v6 >= net.prefix_len(),
        }
    }
    fn gt(&self, other: &IpOrNet) -> bool {
        match other.0 {
            IpNet::V4(net) => self.v4 > net.prefix_len(),
            IpNet::V6(net) => self.v6 > net.prefix_len(),
        }
    }
    fn le(&self, other: &IpOrNet) -> bool {
        match other.0 {
            IpNet::V4(net) => self.v4 <= net.prefix_len(),
            IpNet::V6(net) => self.v6 <= net.prefix_len(),
        }
    }
    fn lt(&self, other: &IpOrNet) -> bool {
        match other.0 {
            IpNet::V4(net) => self.v4 < net.prefix_len(),
            IpNet::V6(net) => self.v6 < net.prefix_len(),
        }
    }
    fn partial_cmp(&self, other: &IpOrNet) -> Option<std::cmp::Ordering> {
        match other.0 {
            IpNet::V4(net) => self.v4.partial_cmp(&net.prefix_len()),
            IpNet::V6(net) => self.v6.partial_cmp(&net.prefix_len()),
        }
    }
}

#[derive(Debug)]
pub struct ParsePrefixlenError {
    msg: &'static str,
}

impl Display for ParsePrefixlenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.msg)
    }
}

impl std::error::Error for ParsePrefixlenError {}

impl FromStr for PrefixlenPair {
    type Err = ParsePrefixlenError;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.split_once(',') {
            Some(pair) => {
                let v4 = u8::from_str(pair.0).or(Err(ParsePrefixlenError {
                    msg: "Unable to parse integer",
                }))?;
                let v6 = u8::from_str(pair.1).or(Err(ParsePrefixlenError {
                    msg: "Unable to parse integer",
                }))?;
                if v4 > 32 || v6 > 128 {
                    return Err(ParsePrefixlenError {
                        msg: "Invalid prefix length",
                    });
                }
                Ok(PrefixlenPair { v4, v6 })
            }
            None => {
                let len = u8::from_str(s).or(Err(ParsePrefixlenError {
                    msg: "Unable to parse integer",
                }))?;
                if len > 128 {
                    return Err(ParsePrefixlenError {
                        msg: "Invalid prefix length",
                    });
                }
                Ok(PrefixlenPair { v4: len, v6: len })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use core::panic;
    use std::net::Ipv6Addr;
    const TEST_V4_ADDR: Ipv4Addr = Ipv4Addr::new(198, 51, 100, 123);
    const TEST_V6_ADDR: Ipv6Addr = Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0x23ab, 0xf007);

    const TEST_V4_NET: Ipv4Net = match Ipv4Net::new(Ipv4Addr::new(192, 0, 2, 0), 24) {
        Ok(net) => net,
        Err(_) => panic!("Couldn't unwrap test vector"),
    };
    const TEST_V6_NET: Ipv6Net =
        match Ipv6Net::new(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0x23ab, 0), 64) {
            Ok(net) => net,
            Err(_) => panic!("Couldn't unwrap test vector"),
        };

    const TEST_V4_ALLNET: Ipv4Net = match Ipv4Net::new(Ipv4Addr::new(0, 0, 0, 0), 0) {
        Ok(net) => net,
        Err(_) => panic!("Couldn't unwrap test vector"),
    };

    const TEST_V6_ALLNET: Ipv6Net = match Ipv6Net::new(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0), 0) {
        Ok(net) => net,
        Err(_) => panic!("Couldn't unwrap test vector"),
    };

    use super::*;
    #[test]
    fn parse_bare_v4() {
        let ip: IpOrNet = "198.51.100.123".parse().unwrap();
        assert_eq!(ip, TEST_V4_ADDR.into());
    }
    #[test]
    fn parse_bare_v6() {
        let ip: IpOrNet = "2001:db8::23ab:f007".parse().unwrap();
        assert_eq!(ip, TEST_V6_ADDR.into());
    }
    #[test]
    fn parse_cidr_v4() {
        let net: IpOrNet = "192.0.2.0/24".parse().unwrap();
        assert_eq!(net, TEST_V4_NET.into());
    }
    #[test]
    fn parse_cidr_v4_min() {
        let net: IpOrNet = "0.0.0.0/0".parse().unwrap();
        assert_eq!(net, TEST_V4_ALLNET.into());
    }
    #[test]
    fn parse_cidr_v4_max() {
        let net: IpOrNet = "198.51.100.123/32".parse().unwrap();
        assert_eq!(net, TEST_V4_ADDR.into());
    }
    #[test]
    fn parse_cidr_v6() {
        let net: IpOrNet = "2001:db8::23ab:0/64".parse().unwrap();
        assert_eq!(net, TEST_V6_NET.into());
    }
    #[test]
    fn parse_cidr_v6_min() {
        let net: IpOrNet = "::/0".parse().unwrap();
        assert_eq!(net, TEST_V6_ALLNET.into());
    }
    #[test]
    fn parse_netmask_v4() {
        let net: IpOrNet = "192.0.2.0/255.255.255.0".parse().unwrap();
        assert_eq!(net, TEST_V4_NET.into());
    }
    #[test]
    fn parse_wildmask_v4() {
        let net: IpOrNet = "192.0.2.0/0.0.0.255".parse().unwrap();
        assert_eq!(net, TEST_V4_NET.into());
    }
    #[test]
    #[should_panic]
    fn reject_v4_mask_v6() {
        let _net: IpOrNet = "2001:db8::23ab:0/255.255.255.0".parse().unwrap();
    }
    #[test]
    #[should_panic]
    fn reject_v6_mask_v6() {
        let _net: IpOrNet = "2001:db8::23ab:0/ffff:ffff:ffff:ffff:ffff:ffff:ffff:0"
            .parse()
            .unwrap();
    }
    #[test]
    #[should_panic]
    fn reject_v4_invalid_pfxlen() {
        let _net: IpOrNet = "192.0.2.0/33".parse().unwrap();
    }
    #[test]
    #[should_panic]
    fn reject_v6_invalid_pfxlen() {
        let _net: IpOrNet = "2001:db8::32ab:0/129".parse().unwrap();
    }
    #[test]
    fn parse_single_prefixlen() {
        let pfxlen: PrefixlenPair = "20".parse().unwrap();
        assert_eq!(pfxlen, PrefixlenPair { v4: 20, v6: 20 });
    }
    #[test]
    fn parse_pair_prefixlen() {
        let pfxlen: PrefixlenPair = "20,32".parse().unwrap();
        assert_eq!(pfxlen, PrefixlenPair { v4: 20, v6: 32 });
    }
    #[test]
    #[should_panic]
    fn reject_single_prefixlen_invalid() {
        let _pfxlen: PrefixlenPair = "129".parse().unwrap();
    }
    #[test]
    #[should_panic]
    fn reject_pair_prefixlen_invalid_v4() {
        let _pfxlen: PrefixlenPair = "33,32".parse().unwrap();
    }
    #[test]
    #[should_panic]
    fn reject_pair_prefixlen_invalid_v6() {
        let _pfxlen: PrefixlenPair = "32,129".parse().unwrap();
    }
    #[test]
    #[should_panic]
    fn reject_single_prefixlen_negative() {
        let _pfxlen: PrefixlenPair = "-32".parse().unwrap();
    }
}
