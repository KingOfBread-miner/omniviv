# Architecture

## Overview

Omniviv is a real-time public transport visualization platform consisting of three main services:

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│    Frontend     │────▶│       API       │────▶│     Martin      │
│    (React)      │◀────│     (Rust)      │     │  (Tile Server)  │
└─────────────────┘     └─────────────────┘     └─────────────────┘
        │                       │
        │    WebSocket          │
        │◀──────────────────────│
                                │
                         ┌──────┴──────┐
                         │   SQLite    │
                         │  Database   │
                         └─────────────┘
                                │
                         ┌──────┴──────┐
                         │  External   │
                         │    APIs     │
                         │ (EFA, OSM)  │
                         └─────────────┘
```

## Services

### Frontend (web/)

- **Framework**: React 19 with TypeScript
- **Build Tool**: Vite 7
- **Styling**: Tailwind CSS 4 with shadcn/ui components
- **Map**: MapLibre GL for 3D map visualization
- **Communication**: REST API + WebSocket for real-time updates

Key features:
- Real-time vehicle position interpolation
- 3D building extrusion
- Dark/light mode support
- Custom map controls (zoom, compass, scale)
- Station popup with departure information

### API (api/)

- **Framework**: Axum 0.8 (Rust async web framework)
- **Runtime**: Tokio
- **Database**: SQLite via SQLx
- **Documentation**: OpenAPI via utoipa with Swagger UI at `/swagger-ui`

#### Modules

```
api/src/
├── api/                 # REST endpoints
│   ├── areas/          # Service area management
│   ├── departures/     # Real-time departure data
│   ├── issues/         # OSM data quality issues
│   ├── routes/         # Transit route geometries
│   ├── stations/       # Station and platform info
│   ├── vehicles/       # Vehicle tracking
│   └── ws.rs           # WebSocket handlers
├── providers/          # External data sources
│   ├── osm.rs          # OpenStreetMap data fetching
│   └── timetables/     # Timetable API integrations
│       └── germany/
│           └── bavaria.rs  # EFA Bavaria API
├── sync/               # Background synchronization
│   ├── mod.rs          # SyncManager orchestration
│   ├── types.rs        # Shared types (Departure, etc.)
│   └── issues.rs       # Issue detection
├── config.rs           # Configuration management
└── main.rs             # Application entry point
```

### Martin (Tile Server)

- **Image**: `ghcr.io/maplibre/martin`
- Serves vector tiles (MBTiles format) for the map
- Serves fonts for map labels
- Configured with caching headers via Traefik

## Data Flow

### Initial Load
1. Frontend loads configuration from `/config.json`
2. Frontend requests station, route, and area data from API
3. API queries SQLite database (populated by sync)
4. Frontend renders map with stations and routes

### Real-time Updates
1. SyncManager fetches departures from EFA API every 30 seconds
2. Vehicle positions are calculated from departure times
3. Updates broadcast via WebSocket (`/api/ws/vehicles`)
4. Frontend interpolates vehicle positions between updates

### OSM Data Sync
1. On startup, API fetches transit data from Overpass API
2. Stations, platforms, stop positions, and routes are stored in SQLite
3. Missing stop references are tracked as issues

## WebSocket Channels

### `/api/ws/vehicles`
Real-time vehicle position updates for the map.

### `/api/ws/backend-diagnostics`
Debug channel showing EFA API request/response logs.

## Database Schema

SQLite stores:
- **areas**: Configured service areas with bounding boxes
- **stations**: Transit stations with coordinates
- **platforms**: Platform nodes within stations
- **stop_positions**: Exact stop locations
- **routes**: Transit routes with geometry
- **route_stops**: Stop sequence for each route

Departures are held in-memory (DepartureStore) with 2-hour expiration.

## Configuration

### API (`config.yaml`)
```yaml
cors_permissive: true  # For development
areas:
    - name: "Augsburg"
      bounding_box:
          south: 48.20
          west: 10.75
          north: 48.48
          east: 11.05
      transport_types:
          - tram
```

### Frontend (`config.json`)
```json
{
    "apiUrl": "http://localhost:3000",
    "martinUrl": "http://localhost:3001"
}
```
