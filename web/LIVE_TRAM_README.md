# Live Tram Web Frontend

A React-based frontend for displaying Augsburg's tram network on an interactive map.

## Features

- Interactive map powered by MapLibre GL
- Vector tiles served from local TileServer GL (port 8080)
- 201 tram stations displayed with names
- Clickable stations with popup information
- Navigation and scale controls

## Prerequisites

Make sure the following services are running:

1. **MapTiler Server** (port 8080):
   ```bash
   cd ../maptiler
   docker compose up -d
   ```

2. **Tram data** should be available at `public/augsburg_tram_data.json`

## Development

### Install dependencies

```bash
pnpm install
```

### Start dev server

```bash
pnpm run dev
```

The app will be available at http://localhost:5174

### Build for production

```bash
pnpm run build
```

### Run tests

```bash
pnpm test
```

## Technologies

- **React 19.x** - UI framework
- **TypeScript 5.9.x** - Type safety
- **Vite 7.x** - Build tool and dev server
- **Tailwind CSS 4.x** - Styling
- **MapLibre GL 5.x** - Map rendering
- **Vitest 4.x** - Testing framework

## Map Configuration

The map is configured to:
- Center on Augsburg (10.898°E, 48.371°N)
- Use vector tiles from http://localhost:8080
- Default zoom level: 12
- Display tram stations in red circles with white borders

## Project Structure

```
web/
├── src/
│   ├── components/
│   │   └── Map.tsx          # Main map component
│   ├── App.tsx               # Root component
│   ├── main.tsx              # Entry point
│   └── index.css             # Global styles
├── public/
│   └── augsburg_tram_data.json  # Tram network data
└── package.json
```

## Future Enhancements

- [ ] Display tram lines as routes on the map
- [ ] Real-time tram position tracking
- [ ] Route planning
- [ ] Tram schedule information
- [ ] Filter stations by tram line
- [ ] Search functionality for stations
