import asyncio
import json
import logging
import os
import shlex
from typing import Annotated

from fastapi import FastAPI, HTTPException, Response
from pydantic import BaseModel, Field, model_validator

MAX_ARGS = 20
MAX_ARG_LENGTH = 2048
MAX_OUTPUT_BYTES = 25 * 1024 * 1024
TIMEOUT_SECONDS = 20
MAX_CONCURRENT_JOBS = int(os.environ.get("MAX_CONCURRENT_JOBS", "8"))

logging.basicConfig(level=os.environ.get("LOG_LEVEL", "INFO"))
logger = logging.getLogger("gallery-dl-worker")

app = FastAPI(docs_url=None, redoc_url=None, openapi_url=None)
job_semaphore = asyncio.Semaphore(MAX_CONCURRENT_JOBS)


class QueryRequest(BaseModel):
    url: Annotated[str, Field(max_length=2048)]
    args: Annotated[list[str], Field(max_length=MAX_ARGS)] = Field(default_factory=list)

    @model_validator(mode="after")
    def validate_request(self):
        if not self.url.startswith("https://"):
            raise ValueError("invalid url")
        if any(len(arg) > MAX_ARG_LENGTH or "\x00" in arg for arg in self.args):
            raise ValueError("invalid arg")
        return self


@app.get("/health")
async def health():
    return {"status": "ok"}


@app.post("/query")
async def query(request: QueryRequest):
    async with job_semaphore:
        output = await run_gallery_dl(request)

    if len(output) > MAX_OUTPUT_BYTES:
        logger.error("gallery-dl output too large: %s bytes", len(output))
        raise HTTPException(status_code=502, detail="gallery-dl output too large")

    try:
        json.loads(output)
    except json.JSONDecodeError as exc:
        logger.error("invalid gallery-dl JSON output: %s", exc)
        raise HTTPException(
            status_code=502, detail="invalid gallery-dl output"
        ) from exc

    return Response(content=output, media_type="application/json")


async def run_gallery_dl(request: QueryRequest) -> bytes:
    command = [
        "gallery-dl",
        "--ignore-config",
        "--quiet",
        "-J",
        *request.args,
        request.url,
    ]
    logger.info("running command: %s", shlex.join(command))
    process = await asyncio.create_subprocess_exec(
        *command,
        stdout=asyncio.subprocess.PIPE,
        stderr=asyncio.subprocess.PIPE,
    )

    try:
        stdout, stderr = await asyncio.wait_for(
            process.communicate(), timeout=TIMEOUT_SECONDS
        )
    except asyncio.TimeoutError as exc:
        process.kill()
        await process.wait()
        logger.error("gallery-dl timed out after %s seconds", TIMEOUT_SECONDS)
        raise HTTPException(status_code=504, detail="gallery-dl timed out") from exc

    if process.returncode != 0:
        logger.error(
            "gallery-dl failed with exit code %s: %s",
            process.returncode,
            stderr.decode("utf-8", errors="replace").strip(),
        )
        raise HTTPException(status_code=502, detail="gallery-dl failed")

    if stderr:
        logger.warning(
            "gallery-dl wrote to stderr: %s",
            stderr.decode("utf-8", errors="replace").strip(),
        )

    return stdout
