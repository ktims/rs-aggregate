use std::{
    error::Error,
    fmt::Display,
    net::{IpAddr, Ipv4Addr},
    str::FromStr,
};

use ipnet::{IpNet, Ipv4Net, Ipv6Net};
use iprange::{IpRange, IpRangeIter};

#[derive(Default)]
pub struct IpBothRange {
    v4: IpRange<Ipv4Net>,
    v6: IpRange<Ipv6Net>,
}

impl IpBothRange {
    pub fn new() -> IpBothRange {
        IpBothRange::default()
    }
    pub fn add(&mut self, net: IpOrNet) {
        match net {
            IpOrNet::IpNet(net) => match net {
                IpNet::V4(v4_net) => drop(self.v4.add(v4_net)),
                IpNet::V6(v6_net) => drop(self.v6.add(v6_net)),
            },
            IpOrNet::IpAddr(addr) => match addr {
                IpAddr::V4(v4_addr) => drop(self.v4.add(v4_addr.into())),
                IpAddr::V6(v6_addr) => drop(self.v6.add(v6_addr.into())),
            },
        }
    }
    pub fn simplify(&mut self) {
        self.v4.simplify();
        self.v6.simplify();
    }
}

pub struct IpBothRangeIter<'a> {
    v4_iter: IpRangeIter<'a, Ipv4Net>,
    v6_iter: IpRangeIter<'a, Ipv6Net>,
    _v4_done: bool,
}

impl<'a> Iterator for IpBothRangeIter<'a> {
    type Item = IpNet;
    fn next(&mut self) -> Option<Self::Item> {
        if self._v4_done {
            match self.v6_iter.next() {
                Some(net) => return Some(net.into()),
                None => return None,
            }
        }
        match self.v4_iter.next() {
            Some(net) => Some(net.into()),
            None => {
                self._v4_done = true;
                match self.v6_iter.next() {
                    Some(net) => Some(net.into()),
                    None => None,
                }
            }
        }
    }
}

impl<'a> IntoIterator for &'a IpBothRange {
    type Item = IpNet;
    type IntoIter = IpBothRangeIter<'a>;
    fn into_iter(self) -> Self::IntoIter {
        IpBothRangeIter {
            v4_iter: self.v4.iter(),
            v6_iter: self.v6.iter(),
            _v4_done: false,
        }
    }
}

pub enum IpOrNet {
    IpNet(IpNet),
    IpAddr(IpAddr),
}

#[derive(Debug, Clone)]
pub struct NetParseError {
    #[allow(dead_code)]
    msg: String,
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
        let mask = p.parse::<Ipv4Addr>();
        match mask {
            Ok(mask) => {
                let intrep: u32 = mask.into();
                let lead_ones = intrep.leading_ones();
                if lead_ones > 0 {
                    if lead_ones + intrep.trailing_zeros() == 32 {
                        Ok(lead_ones.try_into()?)
                    } else {
                        Err(Box::new(NetParseError {
                            msg: "Invalid subnet mask".to_owned(),
                        }))
                    }
                } else {
                    let lead_zeros = intrep.leading_zeros();
                    if lead_zeros + intrep.trailing_ones() == 32 {
                        Ok(lead_zeros.try_into()?)
                    } else {
                        Err(Box::new(NetParseError {
                            msg: "Invalid wildcard mask".to_owned(),
                        }))
                    }
                }
            }
            Err(e) => Err(Box::new(e)),
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
                        msg: "Mask form is not valid for IPv6 address".to_owned(),
                    }))
                }
            }
        }
    }
    pub fn prefix_len(&self) -> u8 {
        match self {
            Self::IpNet(net) => net.prefix_len(),
            Self::IpAddr(addr) => match addr {
                IpAddr::V4(_) => 32,
                IpAddr::V6(_) => 128,
            },
        }
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

impl From<IpNet> for IpOrNet {
    fn from(net: IpNet) -> Self {
        IpOrNet::IpNet(net)
    }
}

impl From<IpAddr> for IpOrNet {
    fn from(addr: IpAddr) -> Self {
        IpOrNet::IpAddr(addr)
    }
}
