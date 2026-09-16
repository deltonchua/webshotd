# webshotd

Web screenshot server: an HTTP service that captures full-page
screenshots with a persistent Chromium instance, resizes them, and
slices them into vertical frames, e.g. as a local web-fetch backend
for agents.

- Single shared browser, launched once, driven over CDP (`chromiumoxide`)
- Image processing via `libvips` (fast, low-memory resize + slicing)
- Bounded concurrency; requests are rejected with `503` when saturated
- Per-request and global timeouts

> Note: designed for **local trusted setups**. It intentionally provides
> **no SSRF protection** (the `url` parameter may target internal
> addresses) and only light resource limiting. Do not expose it to an
> untrusted network without adding these guards.

## Build

Requires a Rust toolchain and `libvips` development headers.

```sh
cargo build --release
```

## Run

```sh
webshotd \
  --chromium-path /usr/bin/chromium \
  --user-data-dir /home/user/.local/share/webshotd/chromium \
  --headless
```

Take screenshots on `GET /v1/screenshot`:

```sh
curl "localhost:7468/v1/screenshot?url=https://example.com&vw=1080&vh=1080&ow=600&frames=10&format=webp&timeout_ms=10000"
```

```json
{"frames":["/tmp/webshotd/example.com/frame_0.webp","/tmp/webshotd/example.com/frame_1.webp"]}
```

Health check: `GET /v1/status`.

### Options

| Option            | Description                                | Default            |
| ----------------- | ------------------------------------------ | ------------------ |
| `--chromium-path` | Path to the chromium executable            |                    |
| `--user-data-dir` | Path to the chromium profile directory     |                    |
| `--headless`      | Run the browser in headless mode           | off                |
| `--incognito`     | Run the browser in incognito mode          | off                |
| `--addr`          | Socket address for the server to listen on | `127.0.0.1:7468`   |
| `--concurrency`   | Maximum number of concurrent browser tasks | `20`               |
| `--timeout-ms`    | Request timeout in milliseconds            | `20000`            |
| `--max-frames`    | Maximum number of frames per page          | `20`               |
| `--image-dir`     | Directory for generated images             | `/tmp/webshotd`    |

### Screenshot parameters

| Parameter   | Description                                     |
| ----------- | ----------------------------------------------- |
| `url`       | Page to capture                                 |
| `vw`        | Viewport width (px)                             |
| `vh`        | Viewport height (px)                            |
| `ow`        | Output width (px, preserves aspect ratio)       |
| `frames`    | Maximum number of vertical frames (capped at `--max-frames`) |
| `format`    | Output image format: `jpeg`, `png`, or `webp`   |
| `timeout_ms`| Per-request timeout (capped at `--timeout-ms`)  |

## Tests

```sh
cargo test
```

Two extra integration tests are marked `#[ignore]` and require
manual setup (paths to a local chromium binary, profile directory
and screenshot input), so they only run when explicitly requested
with `--ignored`

```sh
WEBSHOTD_TEST_CHROMIUM=/usr/bin/chromium \
  WEBSHOTD_TEST_PROFILE=/home/user/.local/share/webshotd/chromium \
  cargo test --test browser -- --ignored
WEBSHOTD_TEST_INPUT=/tmp/sample.webp cargo test --test image -- --ignored
```

## License

This project is licensed under the MIT License. See [LICENSE](LICENSE) for details.
