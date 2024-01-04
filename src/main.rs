mod rx;
use std::collections::HashMap;
use std::net::{UdpSocket, SocketAddr};
use std::thread;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};
use crate::rx::Args;
const PONG_PORT_INC: u16 = 1024;

static mut RECEIVED_LENGTH: usize = 0;
static mut RECEIVED_RECORD: HashMap<u32, (f64, f64)> = HashMap::new();
static mut INIT_TIME: Option<SystemTime> = None;

use clap::Parser;
#[derive(Parser, Debug)]
// #[clap(version = "1.0", author = "Yuki Ueda")]
#[clap(version = "1.0")]
struct Args {
    #[clap(short = 'p', long = "port")]
    port: u16,
    #[clap(short = 'd', long = "duration")]
    duration: u16,
    #[clap(long = "calc-jitter")]
    calc_jitter: bool,
    #[clap(long = "calc-rtt")]
    calc_rtt: bool,
    #[clap(short = 't', long = "tos")]
    tos: u8,
}



fn main() {
    let args = Args::parse();
    let sock = UdpSocket::bind(("0.0.0.0", args.port)).unwrap();
    if args.calc_rtt {
        let pong_port = args.port + PONG_PORT_INC;
        let pong_sock = UdpSocket::bind(("0.0.0.0:{}", pong_port)).unwrap();
    } 

    let trigger = Arc::new(Mutex::new(()));

    println!("waiting ...");
    {
        let trigger = trigger.clone();
        let sock = sock.try_clone().unwrap();
        let pong_port = pong_port.clone();
        let pong_sock = pong_sock.try_clone().unwrap();
        thread::spawn(move || recv_thread(args, sock, pong_port, pong_sock, trigger));
    }

    let mut buffer = [0u8; 10240];
    sock.recv_from(&mut buffer).unwrap();
    if args.calc_jitter {
        let (timestamp, init_seq, _, _, _) = extract(&buffer);
        RECEIVED_RECORD.insert(init_seq, (timestamp, SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs_f64()));
    }
    unsafe { INIT_TIME = Some(SystemTime::now()); }

    if let Some(duration) = args.duration {
        thread::sleep(Duration::from_secs(duration as u64));
    } else if let Some(length) = args.length {
        while unsafe { RECEIVED_LENGTH } < length as usize {
            thread::sleep(Duration::from_millis(10));
        }
    } else {
        panic!("Either [--duration] or [--length] should be specified.");
    }

    // Print completion time or received bytes and average throughput
    if let Some(length) = args.length {
        let duration = SystemTime::now().duration_since(unsafe { INIT_TIME.unwrap() }).unwrap();
        println!("Completion Time: {:.3} s", duration.as_secs_f64());
    } else {
        let duration = args.duration.unwrap();
        println!("Received Bytes: {:.3} MB", unsafe { RECEIVED_LENGTH as f64 / 1024.0 / 1024.0 });
    }

    // Print average jitter if --calc-jitter is specified
    if args.calc_jitter {
        let average_delay_ms
        let average_delay_ms: Vec<f64> = RECEIVED_RECORD.values()
            .filter_map(|record| {
                if let Some(timestamp) = record.0 {
                    Some((SystemTime::now().duration_since(timestamp).unwrap().as_secs_f64()) * 1000.0)
                } else {
                    None
                }
            })
            .collect();

        let average_jitter_ms = average_delay_ms.windows(2)
            .map(|pair| pair[1] - pair[0])
            .sum::<f64>() / (average_delay_ms.len() - 1) as f64;

        println!("Average Jitter: {:.6} ms", average_jitter_ms);
    }
}
