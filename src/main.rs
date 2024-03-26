
use std::net::UdpSocket;
use std::sync::{Arc, Mutex};
use clap::Parser;
use std::io::ErrorKind;



const PONG_PORT_INC: u16 = 1024;

#[derive(Parser)]
struct Args {
    port: u16,
    duration: u32,
    calc_rtt : bool
}

struct RecvData{
    seq_offset: Vec<u32>,
    data_len: u32,
    rx_start_time: u64,
}

impl RecvData{
    fn new() -> Self{
        Self{
            seq_offset: vec![0; 1000000],
            data_len: 0,
            rx_start_time: 0,
        }
    }
}

fn main() {
    let args = Args::parse();
    let recv_data = Arc::new(Mutex::new(RecvData::new()));
    let recv_data_thread = Arc::clone(&recv_data);

    // Extract duration from args
    let duration = args.duration.clone();
    
    let lock = Arc::new(Mutex::new(false));
    let lock_clone = Arc::clone(&lock);
    
    std::thread::spawn(move || {
        recv_thread(args, recv_data_thread, lock_clone);
    });

    
    while !*lock.lock().unwrap() {
        std::thread::sleep(std::time::Duration::from_nanos(100_000) );
    }

    // Sleep for the duration
    std::thread::sleep(std::time::Duration::from_secs(duration as u64));
    let data_len = recv_data.lock().unwrap().data_len;
    println!("Received Bytes: {:.3} MB", data_len as f64/ 1024.0 / 1024.0);
    let rx_duration = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() - recv_data.lock().unwrap().rx_start_time;
    println!("Average Throughput: {:.3} Mbps", (data_len  * 8 ) as f64 / rx_duration as f64 / 1e6);
}

fn recv_thread(args: Args, recv_params: Arc<Mutex<RecvData>>, lock: Arc<Mutex<bool>>){
    let addr = format!("0.0.0.0:{}", args.port);    
    let socket = UdpSocket::bind(&addr).unwrap();
    socket.set_nonblocking(true).unwrap();

    let addr = format!("0.0.0.0:0");
    let pong_socket = UdpSocket::bind(&addr).unwrap();
    pong_socket.set_nonblocking(true).unwrap();
    
    println!("Waiting ...");

    let mut buffer = [0; 10240];
    loop {
        if let Ok((_len, _src_addr)) = socket.recv_from(&mut buffer) {
            *lock.lock().unwrap() = true;
            println!("Start");
            let mut data = recv_params.lock().unwrap();
            data.data_len += _len as u32;
            data.rx_start_time = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
            if !args.calc_rtt {
                break;
            }

            let seq = u32::from_le_bytes(buffer[0..4].try_into().unwrap());
            let _offset = u16::from_le_bytes(buffer[4..6].try_into().unwrap());

            while data.seq_offset.len() <= seq as usize {
                data.seq_offset.push(0);
            }
            data.seq_offset[seq as usize] += 1;
            break;
        }
        else {
            std::thread::sleep(std::time::Duration::from_nanos(100_000) );
        }
    }

    loop {
        let mut buffer = [0; 2048];
        if let Ok((_len, src_addr)) = socket.recv_from(&mut buffer) {
            let mut data = recv_params.lock().unwrap();
            data.data_len += _len as u32;

            if !args.calc_rtt {
                continue;
            }

            let seq = u32::from_le_bytes(buffer[0..4].try_into().unwrap());
            let _offset = u16::from_le_bytes(buffer[4..6].try_into().unwrap());
            let num = u16::from_le_bytes(buffer[10..12].try_into().unwrap());

            while data.seq_offset.len() <= seq as usize {
                data.seq_offset.push(0);
            }
            data.seq_offset[seq as usize] += 1;
            if data.seq_offset[seq as usize] == num as u32 {
                let modified_addr = format!("{}:{}", src_addr.ip(), args.port + PONG_PORT_INC);
                match pong_socket.send_to(&mut buffer[.._len], &modified_addr) {
                    Ok(_) => {},
                    Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                        println!("Send operation would block, retrying later...");
                    }
                    Err(e) => {
                        eprintln!("Error sending data: {}", e);
                    }
                }
            }

        }
    }
}