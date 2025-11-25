# Map Component Structure

This directory contains the refactored Map component split into multiple class-based components.

## File Structure

- **Map.tsx** - Main map component (class-based) that handles MapLibre initialization, tram lines, stations, and vehicles
- **StationPopup.tsx** - Station popup component (class-based) showing station name and platform list
- **PlatformPopup.tsx** - Platform popup component (class-based) showing platform details and real-time departures
- **popupManager.tsx** - Popup management class that handles creating and showing popups with React components
- **types.ts** - TypeScript interfaces shared across all map components
- **vehicleUtils.ts** - Utility functions for vehicle position calculation and geometry interpolation
- **index.ts** - Public exports for the map module

## Component Architecture

All components are class-based React components:

### Map (Main Component)
- Lifecycle: `componentDidMount`, `componentWillUnmount`
- Manages MapLibre map instance
- Loads tram lines, stations, and vehicles
- Handles animation loop for vehicle movement
- Delegates popup creation to PopupManager

### StationPopup
- Displays station name and list of platforms
- Handles platform click to navigate to platform details
- Custom close button

### PlatformPopup
- Lifecycle: `componentDidMount`, `componentWillUnmount`
- Displays platform identifier in a circular badge
- Fetches and displays real-time departures (auto-refreshes every 5 seconds)
- Shows line number, destination, and departure times
- Highlights delays in red
- Provides navigation back to parent station

### PopupManager
- Centralizes popup creation logic
- Manages transitions between station and platform popups
- Handles coordinate calculations for popup positioning

## Usage

Import the Map component from the parent directory:

```tsx
import Map from "./components/Map";

<Map className="h-full w-full" />
```

The existing import path works due to the re-export in `components/Map.tsx`.
