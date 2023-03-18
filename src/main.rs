extern crate ipnet;
extern crate iprange;

mod iputils;
use iputils::{IpBothRange, IpOrNet};

use clio::*;
use std::io::BufRead;

use clap::Parser;

#[derive(Parser)]
#[command(author, version, about, long_about=None)]
struct Args {
    #[clap(value_parser, default_value = "-")]
    input: Input,
    #[arg(
        short,
        long,
        default_value = "128",
        help = "Sets the maximum prefix length for entries read. Longer prefixes will be discarded prior to processing."
    )]
    max_prefixlen: u8,
    #[arg(short, long, help = "truncate IP/mask to network/mask (else ignore)")]
    truncate: bool,
    #[arg(id="4", short, help = "Only output IPv4 prefixes", conflicts_with("6"))]
    only_v4: bool,
    #[arg(id="6", short, help = "Only output IPv6 prefixes", conflicts_with("4"))]
    only_v6: bool,
}

impl Default for Args {
    fn default() -> Self {
        Args {
            input: clio::Input::default(),
            max_prefixlen: 128,
            truncate: false,
            only_v4: false,
            only_v6: false,
        }
    }
}

#[derive(Parser)]

struct IpParseError {
    ip: String,
    problem: String,
}

type Errors = Vec<IpParseError>;

#[derive(Default)]
struct App {
    args: Args,
    prefixes: IpBothRange,
    errors: Errors,
}

impl App {
    fn add_prefix(&mut self, pfx: IpOrNet) {
        // Parser accepts host bits set, so detect that case and error if not truncate mode
        if !self.args.truncate {
            match pfx {
                IpOrNet::IpNet(net) => {
                    if net.addr() != net.network() {
                        eprintln!("ERROR: '{}' is not a valid IP network, ignoring.", net);
                        return;
                    }
                }
                IpOrNet::IpAddr(_) => (),
            }
        }
        if pfx.prefix_len() <= self.args.max_prefixlen {
            self.prefixes.add(pfx);
        }
    }
    fn simplify_input(&mut self) {
        for line in self.args.input.to_owned().lock().lines() {
            for net in line.unwrap().split_whitespace().to_owned() {
                let pnet = net.parse::<IpOrNet>();
                match pnet {
                    Ok(pnet) => self.add_prefix(pnet),
                    Err(e) => {
                        self.errors.push(IpParseError {
                            ip: net.to_string(),
                            problem: e.to_string(),
                        });
                        eprintln!("ERROR: '{}' is not a valid IP network, ignoring.", net);
                    }
                }
            }
        }
        self.prefixes.simplify();
    }

    fn main(&mut self) {
        self.args = Args::parse();

        self.simplify_input();

        for net in &self.prefixes {
            println!("{}", net);
        }
    }
}

fn main() {
    let mut app = App::default();
    app.main();
}
