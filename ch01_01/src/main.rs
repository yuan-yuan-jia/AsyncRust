use std::{fs::File, io::BufRead, process::{self, Command, Stdio}, time::{Duration, Instant}};
use std::io::Write;
use reqwest::Error;

fn fibonacci(n: u64) -> u64 {
    if n == 0 || n == 1 {
        return n;
    }

    fibonacci(n - 1) + fibonacci(n - 2)
}

fn run_processes() {
    let mut process1 = Command::new(std::env::current_exe().unwrap())
                                .arg("task")
                                .arg("1")
                                .spawn()
                                .expect("Failed to start process1");
    let mut process2 = Command::new(std::env::current_exe().unwrap())
                                .arg("task")
                                .arg("6")
                                .spawn()
                                .expect("Failed to start process1");
    process1.wait().expect("Failed to wait for process1");
    process2.wait().expect("Failed to wait for process1");
    println!("Both processes have completed.")
}

fn task(no: usize) {
    println!("Running task... ");
    println!("Task {} completed in process: {}",no,std::process::id())
}
fn main() {
    let pid = process::id();
    println!("process ID: {}",pid);
    let stdin = std::io::stdin();
    let mut lines = stdin.lock().lines();
    loop {
        let line = match lines.next() {
            Some(Ok(line)) => line,
            _ => {
                eprintln!("Failed to read from stdin");
                break;
            }
        };
        println!("Received: {}", line);
    }
}
// 3
/* 
fn main() {
    let child_process_code = r#"
        use std::env; 
         use std::process;  
         use std::thread;  
         use std::time::Duration;  
         fn main() {  
            loop {  
                println!("This is the child process speaking!");  
                thread::sleep(Duration::from_secs(4));  
                let pid = process::id();  
                println!("Child process ID: {}", pid);  
            }  
         }
    "#;

    // Create a temporary file
    let mut temp_dir = std::env::temp_dir();
    temp_dir.push("child_process_code.rs");
    let mut file = File::create(&temp_dir).expect("Failed to create temporary file");

    // Write the Rust code to the temporary file
    file.write_all(child_process_code.as_bytes()).expect("Failed to write child process code to temporary file");
   
    let compile_output = Command::new("rustc")
        .arg("-o")
        .arg("child_process")
        .arg(&temp_dir)
        .output()
        .expect("Failed to compile child process code");

    if !compile_output.status.success() {
        eprintln!("Error during compilation:\n{}", String::from_utf8_lossy(&compile_output.stderr));
        return;
    }

    // Spawn the child process
    let mut child = Command::new("./child_process")
        .stdout(Stdio::inherit())
        .spawn()
        .expect("Failed to spawn child process");

    println!("Child process spawned with PID: {}", child.id());
    // Wait for the child process to finish
    let status = child.wait().expect("Failed to wait for child process");
    println!("Child process terminated with status: {:?}", status);
}
*/
// 2
/* 
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let start = Instant::now();

    if args.len() > 2 && args[1] == "task" {
        let start_task_number = args[2].parse::<usize>().unwrap();
        for i in 1..(start_task_number + 1) {
            task(i);
        }
    } else {
        run_processes();
    }

    if args.len() <= 1 {
        let elapsed = start.elapsed();
        println!("The whole program took: {:?}", elapsed);
    }
}*/
// 1
/* 
#[tokio::main]
async fn main() -> Result<(),Error> {
    let mut threads = Vec::new();

    for i in 0..8 {
        let handle = thread::spawn(move || {
            let result = fibonacci(4000);
            println!("Thread {} result: {}",i,result);
        });
        threads.push(handle);
    }
    for handle in threads {
        let _ = handle.join();
    }
    let url = "https://jsonplaceholder.typicode.com/posts/1";
    let start_time = Instant::now();
    let _ = tokio::join!(
        reqwest::get(url),
        reqwest::get(url),
        reqwest::get(url),
        reqwest::get(url),
        reqwest::get(url),
        reqwest::get(url),
    );
    let elapsed_time = start_time.elapsed();
    println!("Request took {} ms", elapsed_time.as_millis());
    Ok(())

}
*/