use anyhow::{Context as _, Error, Result, bail};
use async_native_tls::TlsStream;
use async_task::{Runnable, Task};
use flume::{Receiver, Sender};
use futures_lite::future;
use http::Uri;
use hyper::{Body, Client, Request, Response};
use once_cell::sync::Lazy;
use smol::{Async, io, prelude::*};
use std::net::{Shutdown, TcpStream, ToSocketAddrs};
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
struct CustomExecutor;

impl<F> hyper::rt::Executor<F> for CustomExecutor
where
    F: Future + Send + 'static,
{
    fn execute(&self, fut: F) {
        spawn_task!(async {
            println!("sending reqeust");
            fut.await;
        })
        .detach();
    }
}
enum CustomStream {
    Plain(Async<TcpStream>),
    Tls(TlsStream<Async<TcpStream>>),
}
#[derive(Clone)]
struct CustomConnector;
impl hyper::service::Service<Uri> for CustomConnector {
    type Response = CustomStream;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<std::result::Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }
    fn call(&mut self, req: Uri) -> Self::Future {
        Box::pin(async move {
            let host = req.host().context("cannot parse host")?;
            match req.scheme_str() {
                Some("http") => {
                    let socket_addr = {
                        let host = host.to_string();
                        let port = req.port_u16().unwrap_or(80);
                        smol::unblock(move || (host.as_str(), port).to_socket_addrs())
                            .await?
                            .next()
                            .context("cannot resolve address")?
                    };
                    let stream = Async::<TcpStream>::connect(socket_addr).await?;
                    Ok(CustomStream::Plain(stream))
                }
                Some("https") => {
                    let socket_addr = {
                        let host = host.to_string();
                        let port = req.port_u16().unwrap_or(443);
                        smol::unblock(move || (host.as_str(), port).to_socket_addrs())
                            .await?
                            .next()
                            .context("cannot resolve address")?
                    };
                    let stream = Async::<TcpStream>::connect(socket_addr).await?;
                    let stream = async_native_tls::connect(host, stream).await?;
                    Ok(CustomStream::Tls(stream))
                }
                scheme => bail!("unsupported sceme: {:?}", scheme),
            }
        })
    }
}
impl tokio::io::AsyncRead for CustomStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        match &mut *self {
            CustomStream::Plain(s) => {
                Pin::new(s)
                    .poll_read(cx, buf.initialize_unfilled())
                    .map_ok(|size| {
                        buf.advance(size);
                    })
            }
            CustomStream::Tls(s) => {
                Pin::new(s)
                    .poll_read(cx, buf.initialize_unfilled())
                    .map_ok(|size| {
                        buf.advance(size);
                    })
            }
        }
    }
}

impl tokio::io::AsyncWrite for CustomStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::result::Result<usize, std::io::Error>> {
        match &mut *self {
            CustomStream::Plain(s) => Pin::new(s).poll_write(cx, buf),
            CustomStream::Tls(s) => Pin::new(s).poll_write(cx, buf),
        }
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<std::result::Result<(), std::io::Error>> {
        match &mut *self {
            CustomStream::Plain(s) => Pin::new(s).poll_flush(cx),
            CustomStream::Tls(s) => Pin::new(s).poll_flush(cx),
        }
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<std::result::Result<(), std::io::Error>> {
        match &mut *self {
            CustomStream::Plain(s) => {
                s.get_ref().shutdown(Shutdown::Write)?;
                Poll::Ready(Ok(()))
            }
            CustomStream::Tls(s) => Pin::new(s).poll_close(cx),
        }
    }
}

impl hyper::client::connect::Connection for CustomStream {
    fn connected(&self) -> hyper::client::connect::Connected {
        hyper::client::connect::Connected::new()
    }
}

async fn fetch(req: Request<Body>) -> anyhow::Result<Response<Body>> {
    Ok(Client::builder()
        .executor(CustomExecutor)
        .build::<_, Body>(CustomConnector)
        .request(req)
        .await?)
}

fn main() {
    let runtime = Runtime::new().with_high_num(2).with_low_num(4);
    runtime.run();
    spawn_task!(BackgroundProcess).detach();
    let future = async  {
    let url = "http://www.rust-lang.org";
        let request = Request::get(url)
            .body(Body::empty())
            .unwrap();
        let response = fetch(request).await.unwrap();
        println!("Response status: {}", response.status());
        let body_bytes = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let html = String::from_utf8(body_bytes.to_vec()).unwrap();
        println!("{}", html);
    };
    let test = spawn_task!(future);
    future::block_on(test);
}
