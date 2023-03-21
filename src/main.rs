extern crate ipnet;
extern crate iprange;

use std::process::exit;

mod iputils;
use iputils::{IpBothRange, IpOrNet, PrefixlenPair};

use clio::*;
use std::io::BufRead;

use clap::Parser;

#[derive(Parser)]
#[command(author, version, about, long_about=None)]
struct Args {
    #[clap(value_parser, default_value = "-")]
    input: Vec<Input>,
    #[structopt(
        short,
        long,
        default_value = "32,128",
        help = "Maximum prefix length for prefixes read. Single value applies to IPv4 and IPv6, comma-separated [IPv4],[IPv6]."
    )]
    max_prefixlen: PrefixlenPair,
    #[arg(short, long, help = "truncate IP/mask to network/mask (else ignore)")]
    truncate: bool,
    #[arg(
        id = "4",
        short,
        help = "Only output IPv4 prefixes",
        conflicts_with("6")
    )]
    only_v4: bool,
    #[arg(
        id = "6",
        short,
        help = "Only output IPv6 prefixes",
        conflicts_with("4")
    )]
    only_v6: bool,
}

impl Default for Args {
    fn default() -> Self {
        Args {
            input: Vec::from([clio::Input::default()]),
            max_prefixlen: PrefixlenPair::default(),
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

// type Errors = Vec<IpParseError>;

#[derive(Default)]
struct App {
    args: Args,
    prefixes: IpBothRange,
    // errors: Errors,
}

impl App {
    fn add_prefix(&mut self, pfx: IpOrNet) {
        // Parser accepts host bits set, so detect that case and error if not truncate mode
        // Note: aggregate6 errors in this case regardless of -4, -6 so do the same
        if !self.args.truncate {
            if pfx.addr() != pfx.network() {
                eprintln!("ERROR: '{}' is not a valid IP network, ignoring.", pfx);
                return;
            }
        }
        // Don't bother saving if we won't display.
        if self.args.only_v4 && pfx.is_ipv6() {
            return;
        } else if self.args.only_v6 && pfx.is_ipv4() {
            return;
        }
        if self.args.max_prefixlen >= pfx {
            self.prefixes.add(pfx);
        }
    }
    fn consume_input(&mut self, input: &mut Input) {
        for line in input.lock().lines() {
            match line {
                Ok(line) => {
                    for net in line.split_whitespace() {
                        let pnet = net.parse::<IpOrNet>();
                        match pnet {
                            Ok(pnet) => self.add_prefix(pnet),
                            Err(_e) => {
                                eprintln!("ERROR: '{}' is not a valid IP network, ignoring.", net);
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("I/O error! {}", e);
                    exit(1);
                }
            }
        }
    }
    fn simplify_inputs(&mut self) {
        let inputs = self.args.input.to_owned();
        for mut input in inputs {
            self.consume_input(&mut input);
        }
        self.prefixes.simplify();
    }

    fn main(&mut self) {
        self.args = Args::parse();

        self.simplify_inputs();

        print!("{}", self.prefixes);
    }
}

fn main() {
    let mut app = App::default();
    app.main();
}
