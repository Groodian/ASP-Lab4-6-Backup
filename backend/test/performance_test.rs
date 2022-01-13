use rust_chat::net::server::Server;
use std::{
    io::{ErrorKind, Read, Write},
    net::TcpStream,
    thread,
    thread::JoinHandle,
    time::{Duration, Instant},
};

// 4 is an optimal test value for a 16 thread cpu
const THREADS_AMOUNT: usize = 4;
const BUFFER_SIZE: usize = 100;
const BUFFER_HEAD: &[u8; 11] = b"RustChat0  ";

macro_rules! PerformanceTest {
    ($function:ident) => {
        println!("-------------------------------------------------------------------");
        println!("\x1b[1;32mRunning test: {}\x1b[0m", stringify!($function));

        let address = "127.0.0.1:4444"
            .parse()
            .expect("Error while parsing address!");

        let mut server = Server::new(THREADS_AMOUNT, address);
        let server_stop = server.start();

        let start_time = Instant::now();
        $function();
        let elapsed = start_time.elapsed();

        server_stop.stop();
        server.join();

        println!(
            "\x1b[1;32mTest: {} finished in {:.3?}\x1b[0m",
            stringify!($function),
            elapsed
        );
        println!("-------------------------------------------------------------------");
    };
}

fn main() {
    PerformanceTest!(test_server_performace_single);
    PerformanceTest!(test_server_performace_single_batch);
    PerformanceTest!(test_server_performace_multi);
    PerformanceTest!(test_server_performace_multi_batch);
    PerformanceTest!(test_server_performace_massive);
    PerformanceTest!(test_server_performace_massive_batch);
}

fn create_stream() -> TcpStream {
    let stream = TcpStream::connect("127.0.0.1:4444").expect("Error while connecting to server!");
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("Error while set read timeout!");

    return stream;
}

fn init_buffer() -> [u8; BUFFER_SIZE] {
    let mut buffer = [0; BUFFER_SIZE];

    for i in 0..11 {
        buffer[i] = BUFFER_HEAD[i];
    }

    return buffer;
}

fn encode(buffer: &mut [u8; BUFFER_SIZE], nonce: u32, reply: bool) -> usize {
    let payload = format!("{{\"nonce\":{},\"reply\":{}}}", nonce, reply).into_bytes();

    let payload_size = payload.len().to_string().into_bytes();
    for i in 0..5 {
        if i < payload_size.len() {
            buffer[11 + i] = payload_size[i];
        } else {
            buffer[11 + i] = b' ';
        }
    }

    for i in 0..payload.len() {
        buffer[16 + i] = payload[i];
    }

    return 16 + payload.len();
}

fn write(
    stream: &mut TcpStream,
    buffer: &mut [u8; BUFFER_SIZE],
    mut nonce: u32,
    messages_amount: u32,
) {
    for _ in 0..messages_amount {
        let out_buffer_size = encode(buffer, nonce, false);
        let mut out_buffer_pos = 0;

        loop {
            match stream.write(&buffer[out_buffer_pos..out_buffer_size]) {
                Ok(0) => break,
                Ok(n) => {
                    out_buffer_pos += n;
                    if out_buffer_pos == out_buffer_size {
                        break;
                    }
                }
                Err(err) => panic!("{}", err),
            }
        }
        nonce += 1;
    }
}

fn read(
    stream: &mut TcpStream,
    buffer: &mut [u8; BUFFER_SIZE],
    read_buffer: &mut [u8; BUFFER_SIZE],
    mut nonce: u32,
    messages_amount: u32,
) {
    let mut total_recv = 0;
    for _ in 0..messages_amount {
        let in_buffer_size = encode(buffer, nonce, true);
        let mut in_buffer_pos = 0;

        loop {
            match stream.read(&mut read_buffer[in_buffer_pos..in_buffer_size]) {
                Ok(0) => break,
                Ok(n) => {
                    in_buffer_pos += n;
                    if in_buffer_pos == in_buffer_size {
                        break;
                    }
                }
                Err(ref err) if err.kind() == ErrorKind::TimedOut => break,
                Err(err) => panic!("{}", err),
            }
        }

        if in_buffer_pos == in_buffer_size {
            assert_eq!(buffer[..in_buffer_size], read_buffer[..in_buffer_size]);
            total_recv += 1;
        }

        nonce += 1;
    }

    assert_eq!(messages_amount, total_recv);
}

fn test_server_performace_single() {
    let mut stream = create_stream();

    let buffer: &mut [u8; BUFFER_SIZE] = &mut init_buffer();
    let read_buffer: &mut [u8; BUFFER_SIZE] = &mut init_buffer();
    let mut nonce: u32 = 0;

    for _ in 0..100000 {
        write(&mut stream, buffer, nonce, 1);

        read(&mut stream, buffer, read_buffer, nonce, 1);

        nonce += 1;
    }
}

fn test_server_performace_single_batch() {
    let mut stream = create_stream();

    let buffer: &mut [u8; BUFFER_SIZE] = &mut init_buffer();
    let read_buffer: &mut [u8; BUFFER_SIZE] = &mut init_buffer();
    let nonce: u32 = 0;

    write(&mut stream, buffer, nonce, 100000);

    read(&mut stream, buffer, read_buffer, nonce, 100000);
}

fn test_server_performace_multi() {
    let mut handels: Vec<JoinHandle<()>> = Vec::new();
    for _ in 0..THREADS_AMOUNT {
        let handle = thread::spawn(|| {
            let mut stream = create_stream();

            let buffer: &mut [u8; BUFFER_SIZE] = &mut init_buffer();
            let read_buffer: &mut [u8; BUFFER_SIZE] = &mut init_buffer();
            let mut nonce: u32 = 0;

            for _ in 0..100000 {
                write(&mut stream, buffer, nonce, 1);

                read(&mut stream, buffer, read_buffer, nonce, 1);

                nonce += 1;
            }
        });
        handels.push(handle);
    }

    for handle in handels {
        handle.join().expect("Error while joining test thread!");
    }
}

fn test_server_performace_multi_batch() {
    let mut handels: Vec<JoinHandle<()>> = Vec::new();

    for _ in 0..THREADS_AMOUNT {
        let handle = thread::spawn(|| {
            let mut stream = create_stream();

            let buffer: &mut [u8; BUFFER_SIZE] = &mut init_buffer();
            let read_buffer: &mut [u8; BUFFER_SIZE] = &mut init_buffer();
            let nonce: u32 = 0;

            write(&mut stream, buffer, nonce, 100000);

            read(&mut stream, buffer, read_buffer, nonce, 100000);
        });
        handels.push(handle);
    }

    for handle in handels {
        handle.join().expect("Error while joining test thread!");
    }
}

fn test_server_performace_massive() {
    let mut handels: Vec<JoinHandle<()>> = Vec::new();
    for _ in 0..THREADS_AMOUNT * 3 {
        let handle = thread::spawn(|| {
            let mut stream = create_stream();

            let buffer: &mut [u8; BUFFER_SIZE] = &mut init_buffer();
            let read_buffer: &mut [u8; BUFFER_SIZE] = &mut init_buffer();
            let mut nonce: u32 = 0;

            for _ in 0..100000 {
                write(&mut stream, buffer, nonce, 1);

                read(&mut stream, buffer, read_buffer, nonce, 1);

                nonce += 1;
            }
        });
        handels.push(handle);
    }

    for handle in handels {
        handle.join().expect("Error while joining test thread!");
    }
}

fn test_server_performace_massive_batch() {
    let mut handels: Vec<JoinHandle<()>> = Vec::new();

    for _ in 0..THREADS_AMOUNT * 3 {
        let handle = thread::spawn(|| {
            let mut stream = create_stream();

            let buffer: &mut [u8; BUFFER_SIZE] = &mut init_buffer();
            let read_buffer: &mut [u8; BUFFER_SIZE] = &mut init_buffer();
            let nonce: u32 = 0;

            write(&mut stream, buffer, nonce, 100000);

            read(&mut stream, buffer, read_buffer, nonce, 100000);
        });
        handels.push(handle);
    }

    for handle in handels {
        handle.join().expect("Error while joining test thread!");
    }
}
