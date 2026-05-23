# Neko Fans Server
[![Better Uptime Badge](https://betteruptime.com/status-badges/v1/monitor/iomh.svg)](https://status.Nekofans.net)

This Server keeps track of how many images were displayed with the [Neko Fans](https://github.com/Meisterlala/NekoFans) plugin.

## API

[![Run in Postman](https://run.pstmn.io/button.svg)](https://app.getpostman.com/run-collection/23047093-1b309b0f-b56c-404e-9ff0-7321b09ae2c2?action=collection%2Ffork&collection-url=entityId%3D23047093-1b309b0f-b56c-404e-9ff0-7321b09ae2c2%26entityType%3Dcollection%26workspaceId%3D3f8f09fb-fa15-4552-bd71-a6644cd4e11e)
[![Open API](https://img.shields.io/badge/Open%20API%203.0.0-try%20it%20out-green?style=for-the-badge&logo=swagger)](https://app.swaggerhub.com/apis-docs/Meisterlala/Neko-Server)


| `https://api.nekofans.net/count_total` Generates the total download count from every user |
| --- |
| ![total](https://api.nekofans.net/count_total) |

|      | `https://api.nekofans.net/count/123` Generates an image with that number |
| ---  | --- |
| 420  | ![420](https://api.nekofans.net/count/420) |
| 69   | ![69](https://api.nekofans.net/count/69) |
| 1337 | ![1337](https://api.nekofans.net/count/1337) |
| 314159 | ![314159](https://api.nekofans.net/count/314159) |

### Gallery query API

`POST /gallery/query` runs a constrained `gallery-dl` JSON query through the internal worker and caches the worker result in Redis.

Request body:

```json
{
  "source": "danbooru",
  "rating": "safe",
  "tags": ["cat_girl"],
  "limit": 25
}
```

Fields:

| Field | Required | Description |
| --- | --- | --- |
| `source` | yes | One of `danbooru`, `gelbooru`, `safebooru`, `konachan`, or `yandere`. |
| `rating` | no | Convenience field for a booru rating tag such as `safe`, `questionable`, or `explicit`. `rating:safe` is also accepted and normalized to `safe`. The worker receives this as a normal `rating:<value>` search tag. |
| `tags` | no | Up to 20 tag strings. Tags may contain ASCII letters, numbers, `_`, `-`, and `:`. Existing clients may still pass rating filters as tags, for example `rating:safe`. |
| `limit` | no | Number of gallery-dl items to request. Defaults to `50`; maximum is `100`. |

At least one query term is required: either `rating` or one `tags` entry.

Response body:

```json
[]
```

The response body is exactly the JSON value returned by `gallery-dl`.

Caching behavior:

| Setting | Default | Description |
| --- | --- | --- |
| `GALLERY_DL_CACHE_TTL_SECONDS` | `900` | Redis result cache TTL for normalized `source` + query terms + `limit`. |
| worker lock TTL | `30` | Short Redis lock used to avoid duplicate worker calls for the same cache key. Concurrent identical misses return HTTP `202`. |

Successful responses are always backed by Redis: either `X-Server-Cache: HIT` or the worker result is written to Redis before returning `X-Server-Cache: MISS`. `X-Server-Cache-Ttl-Seconds` reports the remaining server-side TTL. HTTP responses include `Cache-Control: no-store` because this is a POST endpoint whose response varies by request body; caching is server-side in Redis, not browser/proxy caching.

## The technology used to run the server

Neko Server is written in [Rust](https://www.rust-lang.org/) with the asyc framework [Tokio](https://tokio.rs/) and using [Warp](https://github.com/seanmonstar/warp) as a web server. This is then all bundled in a [Docker container](Dockerfile), which gets automaticly [build](cloudbuild.yaml) with Google Cloud Build. Those images are then pushed to a private Container repository.
