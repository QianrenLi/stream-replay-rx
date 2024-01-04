fn to_rate(x: &str) -> f64 {
    if x.ends_with("KB") {
        x.trim_end_matches("KB").parse::<f64>().unwrap() * 1024.0
    } else if x.ends_with("MB") {
        x.trim_end_matches("MB").parse::<f64>().unwrap() * 1024.0 * 1024.0
    } else if x.ends_with("B") {
        x.trim_end_matches("B").parse::<f64>().unwrap()
    } else {
        panic!("Rate should end with [B|KB|MB].")
    }
}

fn extract(buffer: &[u8]) -> (f64, u32, u16, u16, u8) {
    let (seq, offset, _length, _port, num, indicator, timestamp) =
        unsafe { std::mem::transmute::<[u8; 21], (u32, u16, u16, u16, u8, f64)>(*(buffer.get_unchecked(0)..21).try_into().unwrap()) };
    (timestamp, seq, offset, num, indicator)
}

fn recv_thread(args: Args, sock: UdpSocket, pong_port: u16, pong_sock: UdpSocket, trigger: Arc<Mutex<()>>) {
    let mut seq_offset = vec![];
    loop {
        let mut buffer = [0u8; 2048];
        let (size, addr) = sock.recv_from(&mut buffer).unwrap();
        unsafe { RECEIVED_LENGTH += size };

        if args.calc_jitter {
            let (timestamp, seq, offset, num, indicator) = extract(&buffer[..size]);
            if !RECEIVED_RECORD.contains_key(&seq) {
                RECEIVED_RECORD.insert(seq, (timestamp, SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs_f64()));
            }
            while seq >= seq_offset.len() as u32 {
                seq_offset.push([[num, -1], [0, 0]]);
            }
            if let Some(record) = RECEIVED_RECORD.get(&seq) {
                if indicator % 10 == 0 && offset < seq_offset[seq as usize][0][indicator as usize % 10] {
                    seq_offset[seq as usize][0][indicator as usize % 10] = offset;
                } else if indicator % 10 == 1 && offset > seq_offset[seq as usize][0][indicator as usize % 10] {
                    seq_offset[seq as usize][0][indicator as usize % 10] = offset;
                }
                if args.calc_rtt && indicator >= 10 {
                    seq_offset[seq as usize][1][indicator as usize % 10] = seq_offset[seq as usize][1][1 - indicator as usize % 10] + 1;
                }
                if seq_offset[seq as usize][0][0] - seq_offset[seq as usize][0][1] == 1 {
                    if args.calc_rtt {
                        let duration = SystemTime::now().duration_since(RECEIVED_RECORD.get(&seq).unwrap().0).unwrap();
                        let mut buffer = buffer.to_vec();
                        buffer[10..18].copy_from_slice(&duration.as_secs_f64().to_ne_bytes());
                        buffer[18..26].copy_from_slice(&seq_offset[seq as usize][1][0].to_ne_bytes());
                        buffer[26..34].copy_from_slice(&seq_offset[seq as usize][1][1].to_ne_bytes());
                        let pong_addr = SocketAddr::from((addr.ip(), pong_port));
                        pong_sock.send_to(&buffer, &pong_addr).unwrap();
                    }
                    RECEIVED_RECORD.insert(seq, SystemTime::now().duration_since(RECEIVED_RECORD.get(&seq).unwrap().0).unwrap().as_secs_f64());
                    seq_offset[seq as usize] = [];
                }
            }
        }
    }
}