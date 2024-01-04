
use std::io::{BufRead, Write};
use std::sync::{Arc, Mutex};
use tokio::time::{Duration, Instant};

#[derive(Debug, serde::Deserialize)]
struct PriceResponse {
    price: String,
}

impl PriceResponse {
    fn get_price_as_f64(&self) -> f64 {
        self.price.parse().unwrap_or(0.0)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        print_usage();
        return Ok(());
    }

    match args[1].as_str() {
        "--mode=cache" => {
            println!("Selected mode: Cache");
            if args.len() >= 1 && args[2].starts_with("--times=") {
                let times: u64 = args[2].split('=').nth(1).and_then(|s| s.parse().ok()).unwrap_or(10);
                simulate_distributed_client(times).await?;
            } else {
                println!("Invalid argument for cache mode. Use --times=<seconds>.");
            }
        }
        "--mode=read" => {
            println!("Selected mode: Read");
            read_mode()?;
        }
        _ => {
            println!("Invalid mode. Use cache or read.");
            print_usage();
        }
    }

    Ok(())
}

fn print_usage() {
    println!("Usage:");
    println!("  ./simple --mode=<cache|read> [--times=<seconds>]");
}

async fn simulate_distributed_client(times: u64) -> Result<(), Box<dyn std::error::Error>> {
    let start_time = Instant::now();

    let shared_aggregator_data = Arc::new(Mutex::new(AggregatorData::new()));

    let handles: Vec<_> = (1..=5)
        .map(|i| {
            let shared_aggregator_data_clone = shared_aggregator_data.clone();
            tokio::spawn(simulate_client(i, times, start_time, shared_aggregator_data_clone))
        })
        .collect();

    
    for handle in handles {
        let _ = handle.await?;
    }

    let final_aggregate = shared_aggregator_data.lock().unwrap().calculate_final_aggregate();
    println!("Aggregator: Final aggregate of USD prices of BTC is: {}", final_aggregate);

    write_final_aggregate_to_file(final_aggregate)?;

    Ok(())
}

async fn simulate_client(
    client_id: usize,
    times: u64,
    start_time: Instant,
    shared_aggregator_data: Arc<Mutex<AggregatorData>>,
) -> Result<(), Box<dyn std::error::Error + Send + 'static>> {
    let api_url = "https://api.binance.com/api/v3/ticker/price?symbol=BTCUSDT";
    let client = reqwest::Client::new();

    let mut sum = 0.0;
    let mut count = 0;

    while start_time.elapsed().as_secs() < times {
        let response = client.get(api_url).send().await.map_err(|e| {
            
            Box::new(e) as Box<dyn std::error::Error + Send>
        })?;

        let body = response.text().await.map_err(|e| {
            
            Box::new(e) as Box<dyn std::error::Error + Send>
        })?;

        if let Ok(price) = serde_json::from_str::<PriceResponse>(&body) {
            let price_f64 = price.get_price_as_f64();
            println!("Client {}: Fetched price: {}", client_id, price_f64);

            
            shared_aggregator_data.lock().unwrap().add_average(price_f64);
            sum += price_f64;
            count += 1;
        }

        
        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    let average = sum / count as f64;
    println!("Client {}: Average USD price of BTC is: {}", client_id, average);

 
    shared_aggregator_data.lock().unwrap().add_average(average);

    Ok(())
}

fn read_mode() -> Result<(), Box<dyn std::error::Error>> {
    let file_path = "result.txt";

    match std::fs::metadata(file_path) {
        Ok(metadata) => {
            if metadata.len() == 0 {
                println!("The result.txt file is empty. Run in cache mode first.");
            } else {
                let file = std::fs::File::open(file_path)?;
                let reader = std::io::BufReader::new(file);

                for line in reader.lines() {
                    println!("{}", line?);
                }
            }

            Ok(())
        }
        Err(_) => {
            println!("The result.txt file does not exist. Run in cache mode first.");
            Ok(())
        }
    }
}

fn write_final_aggregate_to_file(final_aggregate: f64) -> Result<(), Box<dyn std::error::Error>> {
    let file_path = "result.txt";

    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(file_path)?;

    writeln!(file, "Final aggregate of USD prices of BTC: {}", final_aggregate)?;

    Ok(())
}

#[derive(Debug)]
struct AggregatorData {
    averages: Vec<f64>,
}

impl AggregatorData {
    fn new() -> Self {
        AggregatorData { averages: Vec::new() }
    }

    fn add_average(&mut self, average: f64) {
        self.averages.push(average);
    }

    fn calculate_final_aggregate(&self) -> f64 {
        if self.averages.is_empty() {
            0.0
        } else {
            self.averages.iter().sum::<f64>() / self.averages.len() as f64
        }
    }
}
