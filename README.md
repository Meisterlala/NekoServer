# Neko Fans Server
[![Better Uptime Badge](https://betteruptime.com/status-badges/v1/monitor/iomh.svg)](https://betteruptime.com/?utm_source=status_badge)

This Server keeps track of how many images were displayed with the [Neko Fans](https://github.com/Meisterlala/NekoFans) plugin.

## API

[![Run in Postman](https://run.pstmn.io/button.svg)](https://app.getpostman.com/run-collection/23047093-1b309b0f-b56c-404e-9ff0-7321b09ae2c2?action=collection%2Ffork&collection-url=entityId%3D23047093-1b309b0f-b56c-404e-9ff0-7321b09ae2c2%26entityType%3Dcollection%26workspaceId%3D3f8f09fb-fa15-4552-bd71-a6644cd4e11e)
[![Open API](https://img.shields.io/badge/Open%20API%203.0.0-try%20it%20out-green?style=for-the-badge&logo=swagger)](https://app.swaggerhub.com/apis/Meisterlala/Neko-Server/1.0.0)

| `https://api.nekofans.net/count_total` Generates the total download count from every user |
| --- |
| ![total](https://api.nekofans.net/count_total) |

|      | `https://api.nekofans.net/count/123` Generates an image with that number |
| ---  | --- |
| 420  | ![420](https://api.nekofans.net/count/420) |
| 69   | ![69](https://api.nekofans.net/count/69) |
| 1337 | ![1337](https://api.nekofans.net/count/1337) |
| 314159 | ![314159](https://api.nekofans.net/count/314159) |

## The technology used to run the server

Neko Server is written in [Rust](https://www.rust-lang.org/) with the asyc framework [Tokio](https://tokio.rs/) and using [Warp](https://github.com/seanmonstar/warp) as a web server. This is then all bundled in a [Docker container](Dockerfile), which gets automaticly [build](cloudbuild.yaml) with Google Cloud Build. Those images are then pushed to a private Container repository.
