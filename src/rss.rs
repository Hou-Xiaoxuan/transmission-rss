use crate::config::{Config, RssList};
use crate::notification::notify_all;
use lava_torrent::torrent::v1::Torrent;
use openssl::base64;
use rss::{Channel, Item};
use sled::Db;
use std::error::Error;
use std::sync::Arc;
use transmission_rpc::types::{BasicAuth, RpcResponse, TorrentAddArgs, TorrentAddedOrDuplicate};
use transmission_rpc::TransClient;
struct TorrentItem {
    pub title: String,
    pub torrent: Torrent,
}
impl TorrentItem {
    pub async fn new(
        url: String,
        title: String,
    ) -> Result<TorrentItem, Box<dyn Error + Send + Sync>> {
        let res = get_with_retry(&url, 1).await;
        if res.is_err() {
            return Err(format!("Failed to fetch the torrent file : {:?}", res).into());
        }
        let res = res.unwrap();
        if res.status() != 200 {
            // return Err(fmt"Failed to fetch the torrent file : {:?}".into());
            return Err(format!(
                "Failed to fetch the torrent file :{:?}; url = {:?}",
                res, url
            )
            .into());
        }
        // let mut buffer: Vec<u8> = Vec::new();
        // res.into_reader().read_to_end(&mut buffer).unwrap();

        // let torrent = Torrent::read_from_bytes(&buffer)?;
        let torrent = Torrent::read_from_bytes(&res.bytes().await?)?;
        Ok(TorrentItem { title, torrent })
    }
}

async fn get_with_retry(
    url: &str,
    retry: u32,
) -> Result<reqwest::Response, Box<dyn Error + Send + Sync>> {
    let mut count = 0;
    loop {
        let res = reqwest::get(url).await;
        if res.is_ok() {
            return Ok(res?);
        }
        count += 1;
        if count > retry {
            return Err(format!("Failed to fetch the torrent file : {:?}", url).into());
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
}

pub async fn process_feed(
    db: Arc<Db>,
    item: RssList,
    cfg: Config,
) -> Result<i32, Box<dyn Error + Send + Sync>> {
    println!("----------------------------");
    println!("==> Processing [{}]", item.title);

    // Fetch the url
    let content = get_with_retry(&item.url, 3).await?.bytes().await?;
    log::info!("[{:?}] feed fetched", item.title);
    let channel = Channel::read_from(&content[..])?;

    let tasks = channel
        .items
        .into_iter()
        .map(|it| {
            let db_copy = db.clone();
            let filters = item.filters.clone();
            async move {
                // TODO vaoid some fetch in new, add some cache or db check
                let it = TorrentItem::new(
                    get_link(&it).to_string(),
                    it.title().unwrap_or_default().to_string(),
                )
                .await;
                if let Err(err) = it {
                    log::warn!("Failed to process item: {}", err);
                    return None;
                }
                let it = it.unwrap();

                // check if item is already on db
                let db_found = match db_copy.get(it.torrent.clone().info_hash()) {
                    Ok(val) => val,
                    Err(_) => None,
                };
                if db_found.is_some() {
                    return None;
                }

                // check filter, if no filter, default to true
                let mut found = true;
                if !filters.is_empty() {
                    found = false;
                    for filter in filters {
                        if it.title.contains(&filter) {
                            found = true;
                        }
                    }

                    if !found {
                        log::debug!("Skipping {} as it doesn't match any filter", it.title)
                    }
                }
                if found {
                    Some(it)
                } else {
                    None
                }
            }
        })
        .collect::<Vec<_>>();

    // let results: Vec<Option<TorrentItem>> = futures::future::join_all(tasks).await;
    let mut results = Vec::new();
    for task in tasks {
        let res = task.await;
        if res.is_some() {
            results.push(res);
        }
    }
    download_torrents(db, item, cfg, results).await
}

async fn download_torrents(
    db: Arc<Db>,
    item: RssList,
    cfg: Config,
    results: Vec<Option<TorrentItem>>,
) -> Result<i32, Box<dyn Error + Send + Sync>> {
    log::info!("[{:?}] [{:?}] torrents found", item.title, results.len());

    // Creates a new connection
    let mut client = get_client(&cfg);

    let mut count = 0;
    for result in results.iter().filter(|it| it.is_some()) {
        let result = result.as_ref().unwrap();
        log::info!("Adding torrent: {}", result.title);
        // Add the torrent into transmission
        let add: TorrentAddArgs = TorrentAddArgs {
            filename: Some(result.torrent.magnet_link().unwrap()),
            download_dir: Some(item.download_dir.clone()),
            ..TorrentAddArgs::default()
        };
        let res = {
            let retry = 3;
            let mut count = 0;
            loop {
                let res: RpcResponse<TorrentAddedOrDuplicate> =
                    client.torrent_add(add.clone()).await?;
                if res.is_ok() {
                    break res;
                }
                count += 1;
                if count > retry {
                    return Err(format!("Failed to add torrent: {}", result.title).into());
                }
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
        };
        if !res.is_ok() {
            log::warn!("Failed to add torrent: {}", result.title);
            continue;
        }

        // check if torrent was added
        match res.arguments {
            TorrentAddedOrDuplicate::TorrentAdded(torrent) => {
                count += 1;
                // send notification
                notify_all(cfg.clone(), format!("Downloading: {}", result.title)).await;
                // Save the hash on the database
                db.insert(torrent.hash_string.unwrap(), b"").unwrap();
            }
            TorrentAddedOrDuplicate::TorrentDuplicate(torrent) => {
                let hash = torrent.hash_string.unwrap();
                log::warn!("Torrent already exists: {}", hash);
                db.insert(hash, b"").unwrap();
            }
        }
    }
    // Persist changes on disk
    db.flush()?;
    log::info!("[{:?}] add【{:?}】 torrents", item.title, count);
    Ok(count)
}

fn get_link(item: &Item) -> &str {
    match item.enclosure() {
        Some(enclosure) if enclosure.mime_type() == "application/x-bittorrent" => enclosure.url(),
        _ => item.link().unwrap_or_default(),
    }
}

pub fn get_client(cfg: &Config) -> TransClient {
    let basic_auth = BasicAuth {
        user: cfg.transmission.username.clone(),
        password: cfg.transmission.password.clone(),
    };
    TransClient::with_auth(cfg.transmission.url.parse().unwrap(), basic_auth)
}

/**Get base64 of content of .torrent file url, incase some url can't be processed bt transmission */
#[allow(dead_code)]
async fn get_metainfo(url: &str) -> Result<String, Box<dyn Error + Send + Sync>> {
    let res = reqwest::get(url).await?;
    // base 64
    if res.error_for_status_ref().is_err() {
        // return Err(fmt"Failed to fetch the torrent file : {:?}".into());
        return Err(format!("Failed to fetch the torrent file : {:?}", url).into());
    }
    let metainfo = base64::encode_block(res.bytes().await?.as_ref());
    Ok(metainfo)
}

#[cfg(test)]
mod test {

    use super::*;

    #[tokio::test]
    async fn test_get_metainfo() {
        let url = "https://bangumi.moe/download/torrent/65cdb20e0050540007eb7b3a/[北宇治字幕组] 葬送的芙莉莲 _ Sousou no Frieren [22][WebRip][1080p][HEVC_AAC][简日内嵌][招募时轴].torrent";
        let metainfo = get_metainfo(url).await;
        metainfo.unwrap();
    }

    #[test]
    fn test_sled() {
        let db = sled::open("./test").unwrap();
        // read all
        for item in db.iter() {
            let (key, _) = item.unwrap();
            println!("{:?}", String::from_utf8(key.to_vec()).unwrap());
        }
    }

    #[tokio::test]
    async fn test_torrent_new() {
        let url = "https://dl.dmhy.org/2022/08/17/d70db7716583224da1684de8fa324822461917aa.torrent";
        let torrent = TorrentItem::new(url.to_string(), "test".to_string()).await;
        torrent.unwrap();
    }

    #[tokio::test]
    async fn test_info_hash() {
        print!("test_info_hash");
        let file = std::fs::read_to_string("config.toml").unwrap();
        let cfg = toml::from_str::<Config>(&file).unwrap();
        let mut client = get_client(&cfg);
        let tor = TorrentItem::new(
            "https://dl.dmhy.org/2022/08/17/d70db7716583224da1684de8fa324822461917aa.torrent"
                .to_string(),
            "test".to_string(),
        )
        .await
        .unwrap();

        let add: TorrentAddArgs = TorrentAddArgs {
            filename: Some(tor.torrent.magnet_link().unwrap()),
            ..TorrentAddArgs::default()
        };

        let res: RpcResponse<TorrentAddedOrDuplicate> =
            tokio_test::block_on(client.torrent_add(add)).unwrap();
        if let TorrentAddedOrDuplicate::TorrentAdded(torrent) = res.arguments {
            assert!(tor.torrent.info_hash() == torrent.clone().hash_string.unwrap());
            println!(
                "hash match: {:?} == {:?}",
                tor.torrent.info_hash(),
                torrent.clone().hash_string.unwrap()
            );
            _ = tokio_test::block_on(client.torrent_remove(vec![torrent.id().unwrap()], true));
        } else {
            panic!("Failed to add torrent");
        }
    }
}
