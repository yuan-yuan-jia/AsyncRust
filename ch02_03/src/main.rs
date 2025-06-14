use std::{
    ptr,
    sync::{Arc, Mutex},
    task::Poll,
    time::Duration,
};
// pin example
struct SelfReferential {
    data: String,
    self_pointer: *const String,
}

impl SelfReferential {
    fn new(data: String) -> Self {
        let mut sr = SelfReferential {
            data,
            self_pointer: ptr::null(),
        };
        sr.self_pointer = &sr.data as *const String;
        sr
    }

    fn print(&self) {
        unsafe {
            println!("{}", *self.self_pointer);
        }
    }
}

#[derive(Debug)]
enum CounterType {
    Increment,
    Decrement,
}
struct SharedData {
    counter: i32,
}
impl SharedData {
    fn increment(&mut self) {
        self.counter += 1;
    }
    fn decrement(&mut self) {
        self.counter -= 1;
    }
}
struct CounterFuture {
    counter_type: CounterType,
    data_reference: Arc<Mutex<SharedData>>,
    count: u32,
}
impl Future for CounterFuture {
    type Output = u32;

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        std::thread::sleep(Duration::from_secs(1));
        let mut guard = match self.data_reference.try_lock() {
            Ok(guard) => guard,
            Err(error) => {
                println!("error for {:?}: {}", self.counter_type, error);
                cx.waker().wake_by_ref();
                return Poll::Pending;
            }
        };
        let value = &mut *guard;
        match self.counter_type {
            CounterType::Increment => {
                value.increment();
                println!("after increment: {}", value.counter);
            }
            CounterType::Decrement => {
                value.decrement();
                println!("after decrement: {}", value.counter);
            }
        }
        std::mem::drop(guard);
        self.count += 1;
        if self.count < 3 {
            cx.waker().wake_by_ref();
            return Poll::Pending;
        }
        Poll::Ready(self.count)
    }
}

#[tokio::main]
async fn main() {
    let mut first = SelfReferential::new("first".to_string());
    let mut second = SelfReferential::new("second".to_string());
    unsafe {
        ptr::swap(&mut first, &mut second);
    }
    first.print();

    // second example: share data in futures
    let shared_data = Arc::new(Mutex::new(SharedData { counter: 0 }));
    let counter_one = CounterFuture {
        counter_type: CounterType::Increment,
        data_reference: shared_data.clone(),
        count: 0,
    };
    let counter_two = CounterFuture {
        counter_type: CounterType::Decrement,
        data_reference: shared_data.clone(),
        count: 0,
    };
    let handle_one = tokio::task::spawn(async move { counter_one.await });
    let handle_two = tokio::task::spawn(async move { counter_two.await });
    let _ = tokio::join!(handle_one, handle_two);
}

/*
共享数据通过tokio进行改写
async fn count(count: u32, data: Arc<tokio::sync::Mutex<SharedData>>,  counter_type: CounterType) -> u32 {
for _ in 0..count {
 let mut data = data.lock().await;
 match counter_type {
 CounterType::Increment => {
 data.increment();
 println!("after increment: {}", data.counter);  },
 CounterType::Decrement => {
 data.decrement();
 println!("after decrement: {}", data.counter);
 }
 }
 std::mem::drop(data);
 std::thread::sleep(Duration::from_secs(1));
 }
 return count
 }
*/
