# transmission-rss

A simple binary that reads a config file with a list of rss torrent items and adds them
to transmission.

-   [x] Telegram notification
-   [x] Concurrent rss fetch and processing
-   [x] Helm chart to deploy in a Kubernetes cluster
-   [x] Docker container to use directly or with docker-compose

```
$ transmission-rss
USAGE:
    transmission-rss --config <CONFIG>

OPTIONS:
    -c, --config <CONFIG>    Path to the config file
    -h, --help               Print help information
    -V, --version            Print version information

```

### Getting started

If you have cargo installed it's possible to install the binary by running:

```
$ cargo install transmission-rss
$ transmission-rss -c config.toml
----------------------------
==> Processing [RSS New Linux Distros]
10 items processed

```

### Config file

Example of `config.toml`:

```toml
[persistence]
path = "/path/to/db/folder"

[transmission]
url = "http://myserver/transmission/rpc"
username = "myusername"
password = "mypassword"

[notification.telegram]
bot_token = 123123:your_token
chat_id = 123123

[notification.feishu]
webhook_file = "/path/to/webhook/webhook.txt"

[[rss_list]]
title = "My List"
url = "https://someweb.site/rss.xml"
filters = ["1080p"]
download_dir = "/downloads/my_folder"
```

The password and telegram bot token can optionally be loaded from separate files by specifying `password_file`/`bot_token_file` instead.

### Docker

It's also possible to run the docker container directly or using `docker-compose.yml`.

```
$ docker run -v ./persistence:/persistence ghcr.io/herlon214/transmission-rss:v0.2.2 -- -c /persistence/config.toml
```
