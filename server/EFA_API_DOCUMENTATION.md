# EFA API Documentation for Augsburg Tram Network

This document provides comprehensive documentation for the Bahnland Bayern EFA (Elektronische Fahrplanauskunft) API, specifically tested for the Augsburg tram network.

## Table of Contents

1. [Overview](#overview)
2. [Base URLs](#base-urls)
3. [API Endpoints](#api-endpoints)
4. [Parameters](#parameters)
5. [Response Structure](#response-structure)
6. [Transport Product Classes](#transport-product-classes)
7. [Station IDs](#station-ids)
8. [Usage Examples](#usage-examples)
9. [Implementation Notes](#implementation-notes)

## Overview

The EFA API provides real-time public transport information including:
- Station/stop search
- Real-time departure and arrival information
- Service alerts and disruptions
- Platform information
- Vehicle routing information

## Base URLs

- **Departure Monitor**: `https://bahnland-bayern.de/efa/XML_DM_REQUEST`
- **Station Finder**: `https://bahnland-bayern.de/efa/XML_STOPFINDER_REQUEST`

## API Endpoints

### 1. Station Search (STOPFINDER_REQUEST)

Search for stations by name.

**URL**: `https://bahnland-bayern.de/efa/XML_STOPFINDER_REQUEST`

**Parameters**:
- `outputFormat=rapidJSON` - Response format
- `type_sf=any` - Search for any location type
- `name_sf={search_term}` - Station name to search for

**Example**:
```
GET https://bahnland-bayern.de/efa/XML_STOPFINDER_REQUEST?outputFormat=rapidJSON&type_sf=any&name_sf=Augsburg%20K%C3%B6nigsplatz
```

### 2. Departure Monitor (DM_REQUEST)

Get departure/arrival information for a specific station.

**URL**: `https://bahnland-bayern.de/efa/XML_DM_REQUEST`

**Required Parameters**:
- `mode=direct` - Direct query mode
- `name_dm={station_id}` - Station ID (e.g., "de:09761:101")
- `type_dm=stop` - Location type
- `outputFormat=rapidJSON` - Response format
- `depType=stopEvents` - Event type

**Optional Parameters**:
- `limit={number}` - Maximum number of results (default: varies, tested up to 30)
- `useRealtime={0|1}` - Include real-time data (1 = yes, 0 = no)
- `includedMeans={class}` - Filter by transport class (4 = tram, 6 = bus)
- `timeSpan={minutes}` - Time window for departures in minutes
- `lineRestriction={line_number}` - Filter by specific line
- `itdDate={YYYYMMDD}` - Specific date
- `itdTime={HHMM}` - Specific time
- `itdDateTimeDepArr={dep|arr}` - "dep" for departures, "arr" for arrivals
- `includeCompleteStopSeq=1` - Include complete stop sequence (not always available)

## Parameters

### Tested and Verified

| Parameter | Values | Description | Working |
|-----------|--------|-------------|---------|
| `mode` | `direct` | Query mode | ✅ |
| `name_dm` | Station ID | Station identifier | ✅ |
| `type_dm` | `stop` | Location type | ✅ |
| `outputFormat` | `rapidJSON` | JSON response format | ✅ |
| `depType` | `stopEvents`, `arrival` | Event type | ✅ |
| `limit` | Number (tested: 3-30) | Result limit | ✅ |
| `useRealtime` | `0`, `1` | Real-time data | ✅ |
| `includedMeans` | `4` (tram), `6` (bus) | Transport filter | ✅ |
| `timeSpan` | Minutes (tested: 60) | Time window | ✅ |
| `lineRestriction` | Line number | Line filter | ⚠️ Partial |
| `itdDate` | `YYYYMMDD` | Specific date | ✅ |
| `itdTime` | `HHMM` | Specific time | ✅ |
| `itdDateTimeDepArr` | `dep`, `arr` | Dep/Arr mode | ✅ |
| `includeCompleteStopSeq` | `1` | Stop sequence | ❌ Not available |

### Notes

- `lineRestriction` appears to return different lines than requested (needs further investigation)
- `includeCompleteStopSeq` doesn't add onward/previous calls in tested responses
- `includedMeans=check:4` returns zero results (incorrect syntax)

## Response Structure

### StopFinder Response

```json
{
  "version": "11.0.6.72",
  "locations": [
    {
      "id": "de:09761:101",
      "isGlobalId": true,
      "name": "Augsburg, Königsplatz",
      "disassembledName": "Königsplatz",
      "coord": [5832015.0, 1212829.0],
      "type": "stop",
      "parent": {
        "id": "placeID:9761000:1",
        "name": "Augsburg",
        "type": "locality"
      }
    }
  ]
}
```

### Departure Monitor Response

```json
{
  "version": "11.0.6.72",
  "systemMessages": [],
  "locations": [
    {
      "id": "de:09761:101",
      "name": "Augsburg, Königsplatz",
      "type": "stop"
    }
  ],
  "stopEvents": [
    {
      "location": {
        "id": "de:09761:101:2:A1",
        "name": "Königsplatz",
        "disassembledName": "Bstg. A1",
        "type": "platform",
        "coord": [5832015.0, 1212829.0],
        "properties": {
          "stopId": "2000101",
          "area": "2",
          "platform": "A1",
          "platformName": "Bstg. A1"
        }
      },
      "departureTimePlanned": "2025-11-24T17:45:00Z",
      "departureTimeEstimated": "2025-11-24T17:46:00Z",
      "departureDelay": null,
      "transportation": {
        "id": "avg:03001: :H:j25",
        "name": "Straßenbahn 1",
        "number": "1",
        "product": {
          "id": 3,
          "class": 4,
          "name": "Straßenbahn",
          "iconId": 4
        },
        "destination": {
          "id": "de:09761:1910",
          "name": "Göggingen",
          "type": "stop"
        },
        "origin": {
          "id": "de:09761:2820",
          "name": "Lechhausen, N. Ostfriedh.",
          "type": "stop"
        }
      },
      "infos": [
        {
          "priority": "normal",
          "id": "25378_AVG",
          "version": 4,
          "type": "lineInfo",
          "infoLinks": [
            {
              "urlText": "Linien 1 und 2 - Eröffnung Christkindlmarkt Augsburg am 24.11.2025",
              "url": "http://...",
              "content": "...",
              "subtitle": "..."
            }
          ]
        }
      ]
    }
  ]
}
```

## Transport Product Classes

Based on testing at Oberhausen Nord P+R:

| Class | Name | Description |
|-------|------|-------------|
| 4 | Straßenbahn | Tram |
| 6 | Bus | Bus |

## Station IDs

Station IDs follow the format: `de:{area_code}:{station_number}`

### Known Augsburg Stations

| Station Name | ID | Lines |
|--------------|-----|-------|
| Königsplatz | `de:09761:101` | 1, 2, 3, 4, 6 |
| Oberhausen Nord P+R | `de:09761:422` | 4 |
| Hauptbahnhof | `de:09761:3` | Multiple |

**Finding Station IDs**: Use the STOPFINDER_REQUEST API to search by name.

## Usage Examples

### Example 1: Get Next 10 Tram Departures with Real-time

```bash
curl -s 'https://bahnland-bayern.de/efa/XML_DM_REQUEST?mode=direct&name_dm=de%3A09761%3A101&type_dm=stop&depType=stopEvents&outputFormat=rapidJSON&limit=10&includedMeans=4&useRealtime=1' | jq '.'
```

### Example 2: Search for a Station

```bash
curl -s 'https://bahnland-bayern.de/efa/XML_STOPFINDER_REQUEST?outputFormat=rapidJSON&type_sf=any&name_sf=Augsburg+K%C3%B6nigsplatz' | jq '.locations[0]'
```

### Example 3: Get Departures for Tomorrow at 8 AM

```bash
curl -s 'https://bahnland-bayern.de/efa/XML_DM_REQUEST?mode=direct&name_dm=de%3A09761%3A101&type_dm=stop&depType=stopEvents&outputFormat=rapidJSON&limit=5&includedMeans=4&itdDate=20251125&itdTime=0800' | jq '.stopEvents'
```

### Example 4: Get Arrivals

```bash
curl -s 'https://bahnland-bayern.de/efa/XML_DM_REQUEST?mode=direct&name_dm=de%3A09761%3A101&type_dm=stop&depType=stopEvents&outputFormat=rapidJSON&limit=5&includedMeans=4&itdDateTimeDepArr=arr&useRealtime=1' | jq '.stopEvents'
```

### Example 5: Extract Specific Information

```bash
# Get line numbers, destinations, and times
curl -s 'https://bahnland-bayern.de/efa/XML_DM_REQUEST?mode=direct&name_dm=de%3A09761%3A101&type_dm=stop&depType=stopEvents&outputFormat=rapidJSON&limit=5&includedMeans=4&useRealtime=1' | jq '.stopEvents[] | {line: .transportation.number, destination: .transportation.destination.name, planned: .departureTimePlanned, estimated: .departureTimeEstimated}'
```

## Implementation Notes

### Time Format
- All times are returned in ISO 8601 format with UTC timezone (Z suffix)
- Example: `2025-11-24T17:45:00Z`
- Convert to local time as needed (Augsburg is UTC+1 in winter, UTC+2 in summer)

### Real-time Data
- `departureTimeEstimated` may be `null` if real-time data is unavailable
- When `null`, use `departureTimePlanned` as fallback
- `departureDelay` field often `null` (delay is calculated from estimated vs. planned)

### Service Alerts
- Returned in the `infos` array within each `stopEvent`
- Multiple info messages can be present
- `priority` levels observed: "normal"
- `infoLinks` array contains detailed information with HTML content

### Coordinates
- Coordinates are returned as `[x, y]` arrays
- Coordinate system appears to be a projected coordinate system (not WGS84 lat/lon)
- Values observed: ~5,825,000 - 5,835,000 (x), ~1,210,000 - 1,215,000 (y)
- Likely EPSG:31467 (DHDN / 3-degree Gauss-Kruger zone 3) or similar

### Platform Information
- Platform details included in `location.properties`
- `platformName` example: "Bstg. A1" (Bahnsteig A1)
- `area` and `platform` fields available

### Rate Limiting
- No explicit rate limiting observed during testing
- Response times: typically < 1 second
- Recommend implementing client-side rate limiting for production use

### Error Handling
- Empty `locations` array when station ID not found
- Empty `stopEvents` array when no departures match criteria
- Check response structure before accessing nested fields

## Rust Implementation

See `src/services/efa.rs` for the complete Rust implementation including:
- Type-safe request/response structures using serde
- `search_stations()` - Search for stations by name
- `get_departures()` - Get departures for a station
- `get_arrivals()` - Get arrivals for a station

Example usage:
```rust
use server::services::efa;

// Search for a station
let stations = efa::search_stations("Augsburg Königsplatz").await?;

// Get departures
let departures = efa::get_departures("de:09761:101", 10, true, true).await?;

// Get arrivals
let arrivals = efa::get_arrivals("de:09761:101", 10, true, true).await?;
```

Run the test example:
```bash
cargo run --example test_efa
```

## Additional Resources

- Official EFA documentation: https://data.wien.gv.at/pdf/wiener-linien-routing.pdf (German)
- GitHub simpleefa library: https://github.com/patrickbr/simpleefa
- Mentz Datenverarbeitung GmbH - Original EFA system developer

## Tested On

- Date: 2025-11-24
- API Version: 11.0.6.72
- Location: Augsburg, Germany
- Transport Network: swa (Stadtwerke Augsburg)
