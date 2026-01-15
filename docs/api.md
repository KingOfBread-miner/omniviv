# API Documentation

## Overview

The Omniviv API provides real-time public transport data including stations, departures, routes, and vehicle tracking. Built with Axum (Rust).

## Base URL

- Development: `http://localhost:3000`
- Production: Configured via deployment

## Interactive Documentation

Swagger UI is available at `/swagger-ui/` when the API is running.

OpenAPI spec: `/api-docs/openapi.json`

## Endpoints

### Areas

#### List Areas
```
GET /api/areas
```

Returns all configured service areas.

#### Get Area
```
GET /api/areas/{id}
```

#### Get Area Stats
```
GET /api/areas/{id}/stats
```

Returns statistics for an area (station count, route count, etc.).

---

### Stations

#### List Stations
```
GET /api/stations
```

Returns all stations with their platforms and stop positions.

**Response:**
```json
{
    "stations": [
        {
            "osm_id": 12345,
            "name": "Königsplatz",
            "ref_ifopt": "de:09761:1234",
            "lat": 48.3657,
            "lon": 10.8945,
            "platforms": [...],
            "stop_positions": [...]
        }
    ]
}
```

---

### Routes

#### List Routes
```
GET /api/routes
```

Returns all transit routes.

#### Get Route
```
GET /api/routes/{id}
```

Returns route details including stops.

#### Get Route Geometry
```
GET /api/routes/{id}/geometry
```

Returns GeoJSON geometry for the route.

---

### Departures

#### List All Departures
```
GET /api/departures
```

Returns all upcoming departures across all stops.

**Response:**
```json
{
    "departures": [
        {
            "stop_ifopt": "de:09761:1234:0:1",
            "line_number": "1",
            "destination": "Lechhausen",
            "planned_time": "2024-01-15T10:30:00+01:00",
            "estimated_time": "2024-01-15T10:31:00+01:00",
            "delay_minutes": 1,
            "event_type": "departure",
            "trip_id": "avms-12345"
        }
    ]
}
```

#### Get Departures by Stop
```
POST /api/departures/by-stop
```

**Request:**
```json
{
    "stop_ifopt": "de:09761:1234:0:1"
}
```

**Response:**
```json
{
    "stop_ifopt": "de:09761:1234:0:1",
    "departures": [...]
}
```

---

### Vehicles

#### Get Vehicles by Route
```
POST /api/vehicles/by-route
```

Returns all vehicles currently operating on a route with their stop sequences.

**Request:**
```json
{
    "route_id": 12345678
}
```

**Response:**
```json
{
    "route_id": 12345678,
    "line_number": "1",
    "vehicles": [
        {
            "trip_id": "avms-12345",
            "line_number": "1",
            "destination": "Lechhausen",
            "origin": "Haunstetten Nord",
            "stops": [
                {
                    "stop_ifopt": "de:09761:1234:0:1",
                    "stop_name": "Königsplatz",
                    "sequence": 5,
                    "lat": 48.3657,
                    "lon": 10.8945,
                    "arrival_time": "2024-01-15T10:29:00+01:00",
                    "arrival_time_estimated": "2024-01-15T10:30:00+01:00",
                    "departure_time": "2024-01-15T10:30:00+01:00",
                    "departure_time_estimated": "2024-01-15T10:31:00+01:00",
                    "delay_minutes": 1
                }
            ]
        }
    ]
}
```

---

### Issues

#### List Issues
```
GET /api/issues
```

Returns detected OSM data quality issues (missing stop references, etc.).

**Response:**
```json
{
    "issues": [
        {
            "issue_type": "MissingStopRef",
            "osm_id": 12345,
            "osm_type": "node",
            "name": "Some Stop",
            "area_name": "Augsburg"
        }
    ]
}
```

---

## WebSocket Endpoints

### Vehicle Updates
```
WS /api/ws/vehicles
```

Real-time vehicle position updates.

**Message format:**
```json
{
    "type": "vehicle_update",
    "vehicles": [
        {
            "trip_id": "avms-12345",
            "line_number": "1",
            "position": {"lat": 48.3657, "lon": 10.8945},
            "heading": 45.0,
            "next_stop": "Königsplatz"
        }
    ]
}
```

### Backend Diagnostics
```
WS /api/ws/backend-diagnostics
```

Debug stream of EFA API requests and responses.

---

## Error Handling

Errors are returned as JSON with appropriate HTTP status codes:

```json
{
    "error": "Route not found"
}
```

Common status codes:
- `200` - Success
- `400` - Bad request (invalid input)
- `404` - Resource not found
- `500` - Internal server error

---

## CORS

CORS is configured via `config.yaml`:

**Development (permissive):**
```yaml
cors_permissive: true
```

**Production (restricted):**
```yaml
cors_permissive: false
cors_origins:
    - "https://omniviv.example.com"
```
