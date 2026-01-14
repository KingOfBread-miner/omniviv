# Omniviv

A real-time public transport visualization platform.

## Screenshots

![Real-time 3D Tram Tracking](docs/screenshots/main-map.png)

![Königsplatz Station View](docs/screenshots/koenigsplatz-station.png)

## Features

### Working

- Real-time tram tracking with smooth position interpolation
- 3D map visualization with extruded buildings
- WebSocket-based live vehicle updates
- Multiple tram lines with distinct colors
- Station and platform markers with labels
- Custom map controls (zoom, compass, scale, attribution)
- Dark/light mode support
- Context menu (copy coordinates, measure distance)
- Rendezvous building highlight at Königsplatz (blue when waiting, green when trams meet)

### Partially Working

- **Collision avoidance**: Basic implementation exists but not fully reliable
- **Rendezvous blinking**: Flashes green when trams are about to depart (timing may need tuning)

### Planned / Not Yet Implemented

- End of line rotation (vehicle turnaround animation at terminus)
- Support for all vehicle types (buses, trains, ferries, etc.)
- Realistic 3D vehicle models at closer zoom levels
- Dark mode for map tiles/style
- 3D terrain with underground tunnel visualization
- Day/night cycle and weather visualization
- Navigation routing between points
- Offline map with cached planned timetables
- Pin stations to sidebar for quick access
- "Leave now" traffic light indicator for optimal departure timing
- First-person driver's seat view
- Multiple city/area support
- General map improvements (POIs, local events, etc.)
- Historical data and statistics

## Architecture

- **API**: Rust-based backend using Axum
- **Web**: React frontend with MapLibre GL

## Quick Start

### Using Docker Compose

```bash
cd deployment
# Configure values.yaml as needed
# Run the deployment script
```

### Development

**API:**
```bash
cd api
cargo run
```

**Web:**
```bash
cd web
pnpm install
pnpm dev
```

## Documentation

See the [docs](docs/) folder for detailed documentation.

## Docker Images

Docker images are automatically built and published to GitHub Container Registry:

- `ghcr.io/firstdorsal/omniviv-api`
- `ghcr.io/firstdorsal/omniviv-frontend`

See [docs/releasing.md](docs/releasing.md) for the full release workflow.

## License

[Add license information here]
