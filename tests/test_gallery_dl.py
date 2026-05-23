#!/usr/bin/env python3

import argparse
import json
import sys
import urllib.error
import urllib.request

WORKER_QUERIES = [
    {
        "name": "worker danbooru range",
        "request": {
            "url": "https://danbooru.donmai.us/posts?tags=rating:safe",
            "args": ["--range", "1-1"],
        },
    },
    {
        "name": "worker safebooru range",
        "request": {
            "url": "https://safebooru.org/index.php?page=post&s=list&tags=cat",
            "args": ["--range", "1-1"],
        },
    },
]

SERVER_QUERIES = [
    {
        "name": "server danbooru query",
        "request": {
            "source": "danbooru",
            "rating": "safe",
            "tags": [],
            "limit": 1,
        },
    },
]


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Run smoke queries against local docker-compose gallery-dl services"
    )
    parser.add_argument(
        "--worker-url",
        default="http://localhost:8090/query",
        help="gallery-dl worker query endpoint",
    )
    parser.add_argument(
        "--server-url",
        default="http://localhost:8080/gallery/query",
        help="neko server gallery query endpoint",
    )
    args = parser.parse_args()

    for query in WORKER_QUERIES:
        response, _ = run_query(args.worker_url, query)
        if response is None:
            return 1
        if not isinstance(response, list):
            print(
                f"failed {query['name']}: expected gallery-dl JSON list",
                file=sys.stderr,
            )
            return 1
        print(f"ok {query['name']}: {len(response)} top-level items")

    for query in SERVER_QUERIES:
        response, headers = run_query(args.server_url, query)
        if response is None:
            return 1
        if not isinstance(response, list):
            print(
                f"failed {query['name']}: expected gallery-dl JSON list",
                file=sys.stderr,
            )
            return 1
        if headers.get("Cache-Control") != "no-store":
            print(
                f"failed {query['name']}: expected no-store Cache-Control",
                file=sys.stderr,
            )
            return 1
        if headers.get("X-Server-Cache") not in {"HIT", "MISS"}:
            print(
                f"failed {query['name']}: expected server cache header", file=sys.stderr
            )
            return 1
        print(f"ok {query['name']}: server-cache={headers.get('X-Server-Cache')}")

    return 0


def run_query(url: str, query: dict):
    print(f"running {query['name']}...")
    try:
        return post_json(url, query["request"])
    except urllib.error.HTTPError as error:
        body = error.read().decode("utf-8", errors="replace")
        print(f"failed {query['name']}: HTTP {error.code}: {body}", file=sys.stderr)
        if error.code == 400 and "invalid tag count" in body:
            print(
                "server appears stale: rebuild/restart nekoserver so rating-only queries are accepted",
                file=sys.stderr,
            )
    except urllib.error.URLError as error:
        print(f"failed {query['name']}: {error}", file=sys.stderr)
    except json.JSONDecodeError as error:
        print(f"failed {query['name']}: invalid JSON: {error}", file=sys.stderr)
    return None, {}


def post_json(url: str, value: dict):
    body = json.dumps(value).encode("utf-8")
    request = urllib.request.Request(
        url,
        data=body,
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    with urllib.request.urlopen(request, timeout=60) as response:
        return json.loads(response.read()), response.headers


if __name__ == "__main__":
    raise SystemExit(main())
