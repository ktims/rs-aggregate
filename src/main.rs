extern crate ipnet;
extern crate iprange;

use clio::*;
use ipnet::{IpNet, Ipv4Net, Ipv6Net};
use iprange::IpRange;
use std::io::BufRead;

use clap::Parser;

#[derive(Parser)]
#[command(author, version, about, long_about=None)]
struct Args {
    #[clap(value_parser, default_value = "-")]
    input: Input,
}

struct IpParseError {
    ip: String,
    problem: String,
}

struct IpBothRange {
    v4: IpRange<Ipv4Net>,
    v6: IpRange<Ipv6Net>,
}

type Errors = Vec<IpParseError>;

fn simplify_input(mut input: Input) -> (IpBothRange, Errors) {
    let mut res = IpBothRange {
        v4: IpRange::new(),
        v6: IpRange::new(),
    };
    let mut errors = Errors::new();
    for line in input.lock().lines() {
        for net in line.unwrap().split_whitespace() {
            match net.parse() {
                Ok(ipnet) => match ipnet {
                    IpNet::V4(v4_net) => {
                        res.v4.add(v4_net.trunc());
                        ()
                    }
                    IpNet::V6(v6_net) => {
                        res.v6.add(v6_net.trunc());
                        ()
                    }
                },
                Err(error) => {
                    eprintln!("ERROR: {} - {}, ignoring.", net, error.to_string());
                    errors.push(IpParseError {
                        ip: net.to_string(),
                        problem: error.to_string(),
                    });
                }
            }
        }
    }
    res.v4.simplify();
    res.v6.simplify();

    (res, errors)
}

fn main() {
    let args = Args::parse();
    let input = args.input;

    let (res, _) = simplify_input(input);

    for net in &res.v4 {
        println!("{}", net);
    }
    for net in &res.v6 {
        println!("{}", net);
    }
}
