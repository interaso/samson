# Samson SMS Daemon

A Rust-based SMS polling daemon that integrates with ModemManager via D-Bus to collect and store SMS messages from connected modems.

## Features

- **Automatic SMS Polling**: Continuously polls connected modems for new SMS messages
- **Database Storage**: Stores SMS messages in SQLite with deduplication
- **REST API**: Query messages by modem IMEI with optional timestamp filtering
- **Metrics Endpoint**: Prometheus-compatible metrics for monitoring
- **Multi-Modem Support**: Handles multiple modems simultaneously
- **D-Bus Integration**: Uses ModemManager for modem communication

## Prerequisites

- Rust 1.70 or later
- ModemManager installed and running
- D-Bus system bus access
- SQLite3

## Installation

```bash
cargo build --release
```

The binary will be available at `target/release/samson`.

## Configuration

Configure the daemon using environment variables:

| Variable        | Description                               | Default     |
|-----------------|-------------------------------------------|-------------|
| `DATABASE_PATH` | Path to SQLite database file              | `samson.db` |
| `POLL_INTERVAL` | Polling interval in seconds (must be > 0) | `1`         |
| `API_HOST`      | Host for main API server                  | `0.0.0.0`   |
| `API_PORT`      | Port for main API server                  | `3030`      |
| `METRICS_HOST`  | Host for metrics/health server            | `0.0.0.0`   |
| `METRICS_PORT`  | Port for metrics/health server            | `9090`      |

## Usage

### Running the daemon

```bash
# With default settings
./samson

# With custom configuration
DATABASE_PATH=/var/lib/samson/sms.db \
POLL_INTERVAL=5 \
API_PORT=8080 \
METRICS_PORT=9091 \
./samson
```

## API Endpoints

### Main API (default port 3000)

#### Get Messages

```
GET /messages/{imei}?after={timestamp}
```

Retrieves SMS messages for a specific modem by IMEI.

**Parameters:**

- `imei` (path, required): Modem IMEI number
- `after` (query, optional): RFC3339 timestamp to filter messages newer than this time

**Response:**

```json
{
  "success": true,
  "data": [
    {
      "id": 1,
      "sender": "+1234567890",
      "text": "Hello world",
      "timestamp": "2026-01-09T08:20:13Z"
    }
  ]
}
```

**Note:** The IMEI field is not included in the response as it's already specified in the URL path.

**Example:**

```bash
# Get all messages for a modem
curl http://localhost:3000/messages/123456789012345

# Get messages after a specific time
curl http://localhost:3000/messages/123456789012345?after=2026-01-09T00:00:00Z
```

### Metrics API (default port 9090)

#### List Modems

```
GET /modems
```

Returns a list of all currently connected modems with their D-Bus paths and IMEI numbers (sorted by path).

**Response:**

```json
{
  "success": true,
  "data": [
    {
      "path": "/org/freedesktop/ModemManager1/Modem/0",
      "imei": "123456789012345"
    },
    {
      "path": "/org/freedesktop/ModemManager1/Modem/1",
      "imei": "987654321098765"
    }
  ]
}
```

#### Prometheus Metrics

```
GET /metrics
```

Returns Prometheus-compatible metrics.

**Response:**

```
# HELP modem_count Total number of modems
# TYPE modem_count gauge
modem_count 2
```

#### Health Check

```
GET /health
```

Simple health check endpoint.

**Response:**

```json
{
  "success": true,
  "data": "OK"
}
```

## Timestamp Format

All timestamps use RFC3339 format. The parser supports both standard format and incomplete timezone offsets:

- Standard: `2026-01-09T08:20:13+01:00`
- Short form: `2026-01-09T08:20:13+01` (automatically converted to `+01:00`)

## Message Deduplication

The daemon automatically prevents duplicate messages from being stored. Messages are considered duplicates if they have the same:

- IMEI
- Sender
- Text content
- Timestamp

## Logging

The daemon uses structured logging via `tracing`. Logs are written to stdout.

## Error Handling

- Invalid timestamps return HTTP 400 Bad Request
- Database errors return HTTP 500 Internal Server Error
- All errors include descriptive messages in the response

## Development

### Running tests

```bash
cargo test
```

### Running with debug logging

```bash
RUST_LOG=debug ./samson
```
