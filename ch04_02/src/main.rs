use async_task::Runnable;
use flume::{Receiver, Sender};
use futures_lite::future;
use once_cell::sync::Lazy;
use smol::Task;
use std::error::Error;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};
use std::{future::Future, panic::catch_unwind, thread};

macro_rules! spawn_task {
    ($future:expr) => {
        spawn_task($future, FutureType::Low)
    };
    ($future:expr,$order:expr) => {
        spawn_task($future, $order)
    };
}

macro_rules! join {
    ($($future:expr),*) => {
        {
            let mut results: Vec<()> = Vec::new();
            $(
                //results.push(future::block_on($future));
                future::block_on($future);
            )*
            results
        }
    }
}

macro_rules! try_join {
    ($($future:expr),*) => {
        {
            let mut results = Vec::new();
            $(
                let result = catch_unwind(||future::block_on($future) )
                results.push(result);
            )*
            results
        }
    }

}

struct Runtime {
    high_num: usize,
    low_num: usize,
}
impl Runtime {
    pub fn new() -> Self {
        let num_cores = std::thread::available_parallelism().unwrap().get();
        Self {
            high_num: num_cores - 2,
            low_num: 1,
        }
    }

    pub fn with_high_num(mut self, num: usize) -> Self {
        self.high_num = num;
        self
    }
    pub fn with_low_num(mut self, num: usize) -> Self {
        self.low_num = num;
        self
    }
    pub fn run(&self) {
        unsafe {
            std::env::set_var("HIGH_NUM", self.high_num.to_string());
            std::env::set_var("LOW_NUM", self.low_num.to_string());
        }
        let high = spawn_task!(async {}, FutureType::High);
        let low = spawn_task!(async {}, FutureType::Low);
        join!(high, low);
    }
}
static HIGH_CHANNEL: Lazy<(Sender<Runnable>, Receiver<Runnable>)> =
    Lazy::new(|| flume::unbounded::<Runnable>());
static LOW_CHANNEL: Lazy<(Sender<Runnable>, Receiver<Runnable>)> =
    Lazy::new(|| flume::unbounded::<Runnable>());

#[derive(Debug, Clone, Copy)]
enum FutureType {
    High,
    Low,
}

trait FutureOrderLable: Future {
    fn get_order(&self) -> FutureType;
}

struct CounterFuture {
    count: u32,
    //order: FutureType,
}
//impl FutureOrderLable for CounterFuture {
//    fn get_order(&self) -> FutureType {
//        self.order
//    }
//}

impl Future for CounterFuture {
    type Output = u32;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.count += 1;
        println!("polling with result: {}", self.count);
        std::thread::sleep(Duration::from_secs(1));
        if self.count < 3 {
            cx.waker().wake_by_ref();
            Poll::Pending
        } else {
            Poll::Ready(self.count)
        }
    }
}
struct AsyncSleep {
    start_time: Instant,
    duration: Duration,
}
impl AsyncSleep {
    fn new(duration: Duration) -> Self {
        Self {
            start_time: Instant::now(),
            duration,
        }
    }
}
impl Future for AsyncSleep {
    type Output = bool;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let elapsed_time = self.start_time.elapsed();
        if elapsed_time >= self.duration {
            Poll::Ready(true)
        } else {
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }
}
async fn async_fn() {
    std::thread::sleep(Duration::from_secs(1));
    println!("async fn");
}

static QUEUE: Lazy<flume::Sender<Runnable>> = Lazy::new(|| {
    let (tx, rx) = flume::unbounded::<Runnable>();
    let queue_one = rx.clone();
    let queue_two = rx.clone();
    thread::spawn(move || {
        while let Ok(runnable) = queue_one.recv() {
            println!("runnable accepted");
            let _ = catch_unwind(|| runnable.run());
        }
    });
    thread::spawn(move || {
        while let Ok(runnable) = queue_two.recv() {
            println!("runnable accepted");
            let _ = catch_unwind(|| runnable.run());
        }
    });
    tx
});
// add feature of stealing task from other queue
static HIGH_QUEUE: Lazy<flume::Sender<Runnable>> = Lazy::new(|| {
    //let (tx,rx) = flume::unbounded::<Runnable>();
    let high_num = std::env::var("HIGH_NUM").unwrap().parse::<usize>().unwrap();
    for _ in 0..high_num {
        let high_receiver = HIGH_CHANNEL.1.clone();
        let low_receiver = LOW_CHANNEL.1.clone();
        thread::spawn(move || {
            //while let Ok(runnable) = receiver.recv() {
            //    println!("[HIGH_QUEUE]: runnable accepted");
            //    let _ = catch_unwind(|| runnable.run());
            //}
            loop {
                match high_receiver.try_recv() {
                    Ok(r) => {
                        let _ = catch_unwind(|| r.run());
                    }
                    Err(_) => match low_receiver.try_recv() {
                        Ok(r) => {
                            let _ = catch_unwind(|| r.run());
                        }
                        Err(_) => {
                            std::thread::sleep(Duration::from_millis(100));
                        }
                    },
                }
            }
        });
    }
    HIGH_CHANNEL.0.clone()
});
/*
static LOW_QUEUE: Lazy<flume::Sender<Runnable>> = Lazy::new(|| {
    let (tx,rx) = flume::unbounded::<Runnable>();
    for _ in 0..1 {
    let receiver = rx.clone();
        thread::spawn(move || {
            while let Ok(runnable) = receiver.recv() {
                println!("[LOW QUEUE]: runnable accepted");
                let _ = catch_unwind(|| runnable.run());
            }
        });
    }
    tx
});
*/
static LOW_QUEUE: Lazy<flume::Sender<Runnable>> = Lazy::new(|| {
    //let (tx,rx) = flume::unbounded::<Runnable>();
    let low_num = std::env::var("LOW_NUM").unwrap().parse::<usize>().unwrap();
    for _ in 0..low_num {
        let high_receiver = HIGH_CHANNEL.1.clone();
        let low_receiver = LOW_CHANNEL.1.clone();
        thread::spawn(move || {
            //while let Ok(runnable) = receiver.recv() {
            //    println!("[HIGH_QUEUE]: runnable accepted");
            //    let _ = catch_unwind(|| runnable.run());
            //}
            loop {
                match low_receiver.try_recv() {
                    Ok(r) => {
                        let _ = catch_unwind(|| r.run());
                    }
                    Err(_) => match high_receiver.try_recv() {
                        Ok(r) => {
                            let _ = catch_unwind(|| r.run());
                        }
                        Err(_) => {
                            std::thread::sleep(Duration::from_millis(100));
                        }
                    },
                }
            }
        });
    }
    LOW_CHANNEL.0.clone()
});
fn spawn_task<F, T>(future: F, order: FutureType) -> Task<T>
where
    F: Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    //let schedule = |runnable| QUEUE.send(runnable).unwrap();
    //let (runnable, task) = async_task::spawn(future, schedule);
    //runnable.schedule();
    //println!("Here is the queue count: {:?}", QUEUE.len());
    //task
    let schedule_high = |runnable| HIGH_QUEUE.send(runnable).unwrap();
    let schedule_low = |runnable| LOW_QUEUE.send(runnable).unwrap();
    let schedule = match order {
        FutureType::High => schedule_high,
        FutureType::Low => schedule_low,
    };
    let (runnable, task) = async_task::spawn(future, schedule);
    runnable.schedule();
    task
}

#[derive(Debug, Clone, Copy)]
struct BackgroundProcess;
impl Future for BackgroundProcess {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        println!("background process firing");
        std::thread::sleep(Duration::from_secs(1));
        cx.waker().wake_by_ref();
        Poll::Pending
    }
}

use mio::net::{TcpListener, TcpStream};
use mio::{Events, Interest, Poll as MioPoll, Token};
use std::io::{Read, Write};

const SERVER: Token = Token(0);
const CLIENT: Token = Token(1);

struct ServerFuture {
    server: TcpListener,
    poll: MioPoll,
}
impl Future for ServerFuture {
    type Output = String;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut events = Events::with_capacity(1);
        let _ = self
            .poll
            .poll(&mut events, Some(Duration::from_millis(200)))
            .unwrap();
        for event in events.iter() {
            if event.token() == SERVER && event.is_readable() {
                let (mut stream, _) = self.server.accept().unwrap();
                let mut buffer = [0u8; 1024];
                let mut received_data = Vec::new();

                loop {
                    match stream.read(&mut buffer) {
                        Ok(n) if n > 0 => {
                            received_data.extend_from_slice(&buffer[..n]);
                        }
                        Ok(_) => {
                            break;
                        }
                        Err(e) => {
                            eprintln!("Error reading from stream: {}", e);
                            break;
                        }
                    }
                }

                if !received_data.is_empty() {
                    let received_str = String::from_utf8_lossy(&received_data);
                    return Poll::Ready(received_str.to_string());
                }
                cx.waker().wake_by_ref();
                return Poll::Pending;
            }
        }
        cx.waker().wake_by_ref();
        Poll::Pending
    }
}
fn main() -> Result<(), Box<dyn Error>> {
    let runtime = Runtime::new().with_high_num(2).with_low_num(4);
    runtime.run();
    let addr = "127.0.0.1:13265".parse()?;
    let mut server = TcpListener::bind(addr)?;
    let mut stream = TcpStream::connect(server.local_addr()?)?;
    let poll: MioPoll = MioPoll::new()?;
    poll.registry()
        .register(&mut server, SERVER, Interest::READABLE)?;
    let server_worker = ServerFuture { server, poll };
    let test = spawn_task!(server_worker);
    let mut client_poll: MioPoll = MioPoll::new()?;
    client_poll
        .registry()
        .register(&mut stream, CLIENT, Interest::WRITABLE)?;
    let mut events = Events::with_capacity(128);
    let _ = client_poll.poll(&mut events, None).unwrap();
    for event in events.iter() {
        if event.token() == CLIENT && event.is_writable() {
            let message = "that's so dingo!\n";
            let _ = stream.write_all(message.as_bytes());
        }
    }
    let outcome = future::block_on(test);
    println!("outcome: {}", outcome);
    Ok(())
}
