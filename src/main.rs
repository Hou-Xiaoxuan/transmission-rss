use clap::Parser;
use sled::Db;
use std::error::Error;
use std::fs;
use std::sync::Arc;
use transmission_rss::config::Config;
use transmission_rss::rss::{get_client, process_feed};

/// Parse args
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Path to the config file
    #[clap(short, long)]
    config: String,
}

pub async fn init_db(cfg: &Config) -> Result<Arc<Db>, Box<dyn Error + Send + Sync>> {
    let db = sled::open(&cfg.persistence.path)?;
    if db.was_recovered() {
        log::info!("Database recovered");
    }

    // fetch updated torrents
    let mut client = get_client(cfg);
    let res = client.torrent_get(None, None).await;
    if res.is_err() {
        return Err(format!(
            "Failed to fetch torrents: {:?}, please check transmission server.",
            res
        )
        .into());
    }
    let res = res.unwrap();
    let mut update_count = 0;
    for torrent in res.arguments.torrents {
        let torrent_hash = torrent.hash_string.unwrap();
        if let Ok(rt) = db.get(&torrent_hash) {
            if rt.is_some() {
                continue;
            }
        }
        update_count += 1;
        db.insert(&torrent_hash, b"").unwrap();
    }
    log::info!("update db with {:?} items", update_count);
    Ok(Arc::new(db))
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
    let db: Arc<Db> = init_db(&cfg).await.unwrap();

    let items: Vec<_> = cfg
        .clone()
        .rss_list
        .into_iter()
        .map(|it| async {
            let title = it.title.clone();
            let rt = process_feed(db.clone(), it, cfg.clone()).await;
            if let Err(err) = rt {
                let msg = format!("Failed to process {} feed: {}", title, err);
                log::error!("{}", msg);
                // notify_all(cfg.clone(), msg).await;
            }
        })
        .collect();
    // _ = futures::future::join_all(items).await;
    for item in items {
        item.await;
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;
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

    #[tokio::test]
    async fn test_init_db() {
        let file = std::fs::read_to_string("config.toml").unwrap();
        let cfg = toml::from_str::<Config>(&file).unwrap();
        let _ = env_logger::builder()
            .filter_level(log::LevelFilter::Info)
            .is_test(true)
            .try_init();
        init_db(&cfg).await.unwrap();
    }
}
