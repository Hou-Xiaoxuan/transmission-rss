use clap::Parser;
use std::fs;
use transmission_rss::config::Config;
use transmission_rss::rss::process_feed;

/// Parse args
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Path to the config file
    #[clap(short, long)]
    config: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .is_test(true)
        .try_init();

    // Read env
    let args = Args::parse();

    // Read initial config file
    let file = match fs::read_to_string(&args.config) {
        Ok(val) => val,
        Err(err) => panic!("Failed to find file {:?}: {}", args.config, err),
    };
    let cfg: Config = toml::from_str(&file).unwrap();

    let items: Vec<_> = cfg
        .clone()
        .rss_list
        .into_iter()
        .map(|it| process_feed(it, cfg.clone()))
        .collect();

    for item in items {
        match item.await {
            Ok(count) => {
                println!("{:?} items processed", count);
            }
            Err(err) => {
                println!("{:?}", err);
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod test {
    #[test]
    fn test_logger() {
        let _ = env_logger::builder()
            .filter_level(log::LevelFilter::Debug)
            .is_test(true)
            .try_init();
        log::info!("info");
        log::debug!("debug");
        log::warn!("warn");
        log::error!("error");
        println!("over");
    }
}
