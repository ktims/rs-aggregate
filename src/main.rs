extern crate ipnet;

use std::{io, process::exit};

mod iputils;
use iputils::{IpBothRange, IpOrNet, PrefixlenPair};

use clio::*;
use std::io::{BufRead, Write};

use clap::Parser;

const WRITER_BUFSIZE: usize = 1 * 1024;

#[derive(Parser)]
#[command(author, version, about)]
struct Args {
    #[clap(value_parser, default_value = "-")]
    input: Vec<Input>,
    /// Maximum prefix length for prefixes read. Single value applies to IPv4 and IPv6, comma-separated [IPv4],[IPv6].
    #[structopt(short, long, default_value = "32,128")]
    max_prefixlen: PrefixlenPair,
    /// Truncate IP/mask to network/mask (else ignore)
    #[arg(short, long)]
    truncate: bool,
    /// Only output IPv4 prefixes
    #[arg(id = "4", short, conflicts_with("6"))]
    only_v4: bool,
    /// Only output IPv6 prefixes
    #[arg(id = "6", short, conflicts_with("4"))]
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

        let stdout = io::stdout().lock();
        let mut w = io::BufWriter::with_capacity(WRITER_BUFSIZE, stdout);

        write!(&mut w, "{}", self.prefixes).unwrap();
        w.flush().unwrap();
    }
}

fn main() {
    let mut app = App::default();
    app.main();
}
