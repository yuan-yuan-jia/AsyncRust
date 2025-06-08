use async_task::{Runnable, Task};
use flume::{Receiver, Sender};
use futures_lite::future;
use once_cell::sync::Lazy;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};
use std::{future::Future, panic::catch_unwind, thread};

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
    for _ in 0..2 {
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
    for _ in 0..1 {
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

fn main() {
    /*
    let async_sleep = AsyncSleep::new(Duration::from_secs(5));
    let async_sleep_handle = spawn_task(async {
        async_sleep.await;
        todo!()
    });
    */
    /*
    let one = CounterFuture {count: 0};
    let two = CounterFuture {count: 0};
    let t_one = spawn_task(one);
    let t_two = spawn_task(two);
    let t_three = spawn_task(async {
        async_fn().await;
        async_fn().await;
        async_fn().await;
        async_fn().await;
    });
    std::thread::sleep(Duration::from_secs(5));
    println!("before the block");
    future::block_on(t_one);
    future::block_on(t_two);
    future::block_on(t_three);
    */
    //let one = CounterFuture {count: 0, order: FutureType::High};
    //let two = CounterFuture {count: 0, order: FutureType::Low};
    //let t_one = spawn_task(one);
    //let t_two = spawn_task(two);

    //future::block_on(t_one);
    //future::block_on(t_two);
    let one = CounterFuture { count: 0 };
    let two = CounterFuture { count: 0 };
    let t_one = spawn_task!(one, FutureType::High);
    let t_two = spawn_task!(two);
    let t_three = spawn_task!(async_fn());
    let t_four = spawn_task!(
        async {
            async_fn().await;
            async_fn().await;
        },
        FutureType::High
    );

    //future::block_on(t_one);
    //future::block_on(t_two);
    //future::block_on(t_three);
    //future::block_on(t_four);
    join!(t_one, t_two, t_three, t_four);
}
