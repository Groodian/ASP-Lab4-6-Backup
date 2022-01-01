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
const OUT_MESSAGE: &[u8; 42] = b"RustChat0  26   {\"message\":\"Hello World!\"}";
const IN_MESSAGE: &[u8; 48] = b"RustChat0  32   {\"message\":\"Hello from Server!\"}";

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

fn write(stream: &mut TcpStream, messages_amount: u32) {
    for _ in 0..messages_amount {
        let mut out_buffer_pos = 0;
        loop {
            match stream.write(&OUT_MESSAGE[out_buffer_pos..]) {
                Ok(0) => break,
                Ok(n) => {
                    out_buffer_pos += n;
                    if out_buffer_pos == 42 {
                        break;
                    }
                }
                Err(err) => panic!("{}", err),
            }
        }
    }
}

fn read(stream: &mut TcpStream, messages_amount: u32) {
    let mut total_recv = 0;
    for _ in 0..messages_amount {
        let in_buffer: &mut [u8; 48] = &mut [0; 48];
        let mut in_buffer_pos = 0;
        loop {
            match stream.read(&mut in_buffer[in_buffer_pos..]) {
                Ok(0) => break,
                Ok(n) => {
                    in_buffer_pos += n;
                    if in_buffer_pos == 48 {
                        break;
                    }
                }
                Err(ref err) if err.kind() == ErrorKind::TimedOut => break,
                Err(err) => panic!("{}", err),
            }
        }

        if in_buffer_pos == 48 {
            assert_eq!(IN_MESSAGE, in_buffer);
            total_recv += 1;
        }
    }

    assert_eq!(messages_amount, total_recv);
}

fn test_server_performace_single() {
    let mut stream = create_stream();

    for _ in 0..100000 {
        write(&mut stream, 1);

        read(&mut stream, 1);
    }
}

fn test_server_performace_single_batch() {
    let mut stream = create_stream();

    write(&mut stream, 100000);

    read(&mut stream, 100000);
}

fn test_server_performace_multi() {
    let mut handels: Vec<JoinHandle<()>> = Vec::new();
    for _ in 0..THREADS_AMOUNT {
        let handle = thread::spawn(|| {
            let mut stream = create_stream();

            for _ in 0..100000 {
                write(&mut stream, 1);

                read(&mut stream, 1);
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

            write(&mut stream, 100000);

            read(&mut stream, 100000);
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

            for _ in 0..100000 {
                write(&mut stream, 1);

                read(&mut stream, 1);
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

            write(&mut stream, 100000);

            read(&mut stream, 100000);
        });
        handels.push(handle);
    }

    for handle in handels {
        handle.join().expect("Error while joining test thread!");
    }
}
