# Neko Fans Server

This Server keeps track of how many images were displayed with the [Neko Fans](https://github.com/Meisterlala/NekoFans) plugin.

# API
[![Run in Postman](https://run.pstmn.io/button.svg)](https://app.getpostman.com/run-collection/23047093-1b309b0f-b56c-404e-9ff0-7321b09ae2c2?action=collection%2Ffork&collection-url=entityId%3D23047093-1b309b0f-b56c-404e-9ff0-7321b09ae2c2%26entityType%3Dcollection%26workspaceId%3D3f8f09fb-fa15-4552-bd71-a6644cd4e11e)

| `http://34.149.0.8/count_total` Generates the total download count from every user |
| --- |
| ![](http://34.149.0.8/count_total) |

|      | `http://34.149.0.8/count/123` Generates an image with that number |
| ---  | --- |
| 420  | ![](http://34.149.0.8/count/420) |
| 69   | ![](http://34.149.0.8/count/69) |
| 1337 | ![](http://34.149.0.8/count/1337) |
| 314159 | ![](http://34.149.0.8/count/314159) |

# The technology used to run the server

Neko Server is written in [Rust](https://www.rust-lang.org/) with the asyc framework [Tokio](https://tokio.rs/) and using [Warp](https://github.com/seanmonstar/warp) as a web server. This is then all bundled in a [Docker container](Dockerfile), which gets automaticly [build](cloudbuild.yaml) with Google Cloud Build. Those images are then pushed to a private Container repository. 

The images get deployed to a [Kubernetes](https://kubernetes.io/) cluter. The SQLite Database is attatched as a PersistentVolumeClaim which is hosted on Google Filestore.

All this together allows for almost infinite scaling and zero downtime while pushing new changes.

### TODO
Get a real Domain instead of a static IP
