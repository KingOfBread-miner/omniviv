# Live Tram - Augsburg Tram Tracking System

A real-time tram tracking application for Augsburg, Germany.

## Project Structure

```
live-tram/
├── server/           # Rust backend server
│   ├── src/
│   │   └── main.rs  # OSM data fetcher for Augsburg tram network
│   └── augsburg_tram_data.json  # Downloaded tram lines and stations
├── maptiler/        # Map tile server
│   ├── docker-compose.yml
│   ├── maptiler-osm-2020-02-10-v3.11-germany_bayern.mbtiles
│   └── README.md
└── web/             # Frontend application (to be implemented)
```

## Components

### 1. Server (Rust)

Downloads and processes Augsburg tram data from OpenStreetMap:
- 32 tram line variants (lines 1, 2, 3, 4, 6, 8, 9)
- 201 tram stations with GPS coordinates
- Route information including stops and geometry

**Run:**
```bash
cd server
cargo run
```

### 2. MapTiler Server (Docker)

Serves Bavaria region map tiles for the frontend:
- Vector tiles in MBTiles format
- Covers entire Bavaria region including Augsburg
- Runs on port 8080

**Run:**
```bash
cd maptiler
docker compose up -d
```

**Access:**
- UI: http://localhost:8080
- Health: http://localhost:8080/health

See [maptiler/README.md](maptiler/README.md) for detailed usage.

### 3. Web Frontend (Coming Soon)

Will display:
- Interactive map with Augsburg tram network
- Real-time tram positions
- Station information
- Route planning

## Quick Start

1. **Download tram data:**
   ```bash
   cd server
   cargo run
   ```

2. **Start map tile server:**
   ```bash
   cd maptiler
   docker compose up -d
   ```

3. **Verify tile server is running:**
   ```bash
   curl http://localhost:8080/health
   ```

## Data Sources

- **Tram Network**: OpenStreetMap via Overpass API
- **Map Tiles**: MapTiler OpenMapTiles (Bavaria extract)

## Technology Stack

- **Backend**: Rust
- **Map Tiles**: TileServer GL (Docker)
- **Data**: OpenStreetMap, MBTiles
- **Frontend**: TBD (MapLibre GL recommended)

## Development

The project is currently in the data gathering phase. Next steps:
1. Create web frontend with MapLibre GL
2. Display tram lines and stations on the map
3. Add real-time tram position tracking
4. Implement route planning features
