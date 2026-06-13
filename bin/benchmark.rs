use std::net::{TcpStream, ToSocketAddrs};
use std::io::{Write, Read};
use std::time::{Instant, Duration};
use std::sync::atomic::{AtomicU64, AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

fn send_request(stream: &mut TcpStream) {
    let req: &[u8] = br#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"execute_query","arguments":{"sql":"SELECT 1"}},"id":0}"#;
    let _ = stream.write_all(req);
    let _ = stream.write_all(b"\n");
    let mut buf = [0u8; 4096];
    loop {
        match stream.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                if buf[..n].iter().any(|&b| b == b'\n') {
                    break;
                }
            }
        }
    }
}

fn client_loop(addr: std::net::SocketAddr, counter: Arc<AtomicU64>, running: Arc<AtomicBool>) {
    while running.load(Ordering::Relaxed) {
        let mut stream = match TcpStream::connect_timeout(&addr, Duration::from_millis(2000)) {
            Ok(s) => s,
            Err(_) => continue,
        };
        stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
        // Send many requests per connection
        for _ in 0..100 {
            if !running.load(Ordering::Relaxed) {
                return;
            }
            counter.fetch_add(1, Ordering::Relaxed);
            send_request(&mut stream);
        }
    }
}

fn main() {
    let addr = "127.0.0.1:3000".to_socket_addrs().unwrap().next().unwrap();
    let args: Vec<String> = std::env::args().collect();
    let duration_secs: u64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(10);
    let concurrency: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(10);

    let counter = Arc::new(AtomicU64::new(0));
    let running = Arc::new(AtomicBool::new(true));
    let start = Instant::now();
    let _end = start + Duration::from_secs(duration_secs);

    let mut handles = vec![];
    for _ in 0..concurrency {
        let counter = Arc::clone(&counter);
        let running = Arc::clone(&running);
        let addr = addr;
        handles.push(thread::spawn(move || {
            client_loop(addr, counter, running);
        }));
    }

    // Sleep for duration, then signal stop
    thread::sleep(Duration::from_secs(duration_secs));
    running.store(false, Ordering::Relaxed);

    for h in handles {
        let _ = h.join();
    }

    let elapsed = start.elapsed();
    let total = counter.load(Ordering::Relaxed);
    let rps = total as f64 / elapsed.as_secs_f64();

    println!();
    println!("=== Results ===");
    println!("Concurrency: {}", concurrency);
    println!("Duration: {:.1}s", elapsed.as_secs_f64());
    println!("Total Requests: {}", total);
    println!("Requests/sec: {:.0}", rps);
    println!("Avg Latency: {:.1}µs", 1_000_000.0 / rps);
}
