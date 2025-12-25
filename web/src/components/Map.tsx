import maplibregl from "maplibre-gl";
import "maplibre-gl/dist/maplibre-gl.css";
import { useCallback, useEffect, useRef, useState } from "react";
import { createRoot, type Root } from "react-dom/client";
import type { Area, Station, StationPlatform, StationStopPosition } from "../api";
import type { RouteVehicles, RouteWithGeometry } from "../App";
import { getPlatformDisplayName } from "./mapUtils";
import { PlatformPopup } from "./PlatformPopup";
import { StationPopup } from "./StationPopup";
import { VehiclePopup } from "./VehiclePopup";
import { calculateVehiclePosition, type VehiclePosition } from "./vehicleUtils";

// Use environment variable or fallback to localhost for development
const MAP_STYLE_URL = import.meta.env.VITE_MAP_STYLE_URL ?? "/styles/basic-preview/style.json";

// Animation frame rate (how often to recalculate positions in ms)
// 50ms = 20fps, good balance of smoothness and performance
const ANIMATION_INTERVAL = 50;

// Vehicle marker icon settings - using high resolution for crisp rendering
const ICON_SIZE = 48; // Base size in pixels (will be scaled down by icon-size)
const ICON_SCALE = 0.5; // Scale factor for display

/**
 * Generate a circle icon with line number for a vehicle marker
 */
function createVehicleIcon(color: string, lineNumber: string): ImageData {
    const size = ICON_SIZE;
    const canvas = document.createElement("canvas");
    canvas.width = size;
    canvas.height = size;
    const ctx = canvas.getContext("2d")!;

    const center = size / 2;
    const radius = size / 2 - 5;

    // Draw white stroke/border
    ctx.beginPath();
    ctx.arc(center, center, radius + 3, 0, Math.PI * 2);
    ctx.fillStyle = "#ffffff";
    ctx.fill();

    // Draw colored circle
    ctx.beginPath();
    ctx.arc(center, center, radius, 0, Math.PI * 2);
    ctx.fillStyle = color;
    ctx.fill();

    // Draw line number text
    ctx.fillStyle = "#ffffff";
    ctx.font = `bold ${size * 0.45}px "Open Sans", sans-serif`;
    ctx.textAlign = "center";
    ctx.textBaseline = "middle";
    ctx.fillText(lineNumber, center, center + 1);

    return ctx.getImageData(0, 0, size, size);
}

interface MapProps {
    areas: Area[];
    stations: Station[];
    routes: RouteWithGeometry[];
    vehicles: RouteVehicles[];
    showAreaOutlines: boolean;
    showStations: boolean;
    showRoutes: boolean;
    showVehicles: boolean;
}

export default function Map({ areas, stations, routes, vehicles, showAreaOutlines, showStations, showRoutes, showVehicles }: MapProps) {
    const mapContainer = useRef<HTMLDivElement>(null);
    const map = useRef<maplibregl.Map | null>(null);
    const popup = useRef<maplibregl.Popup | null>(null);
    const popupRoot = useRef<Root | null>(null);
    const stationsRef = useRef<Station[]>(stations);
    const routeColorsRef = useRef<globalThis.Map<string, string>>(new globalThis.Map());
    const routeGeometriesRef = useRef<globalThis.Map<number, number[][][]>>(new globalThis.Map());
    const vehiclesRef = useRef<RouteVehicles[]>(vehicles);
    const animationRef = useRef<number | null>(null);
    const lastAnimationTimeRef = useRef<number>(0);
    const vehicleIconsRef = useRef<Set<string>>(new Set());
    const [mapLoaded, setMapLoaded] = useState(false);

    // Keep stationsRef in sync with stations prop
    useEffect(() => {
        stationsRef.current = stations;
    }, [stations]);

    // Keep vehiclesRef in sync with vehicles prop
    useEffect(() => {
        vehiclesRef.current = vehicles;
    }, [vehicles]);

    // Build route colors map and geometries map from routes
    useEffect(() => {
        const colorMap = new globalThis.Map<string, string>();
        const geometryMap = new globalThis.Map<number, number[][][]>();
        for (const route of routes) {
            if (route.ref && route.color) {
                colorMap.set(route.ref, route.color);
            }
            if (route.geometry?.segments) {
                geometryMap.set(route.osm_id, route.geometry.segments);
            }
        }
        routeColorsRef.current = colorMap;
        routeGeometriesRef.current = geometryMap;
    }, [routes]);

    // Helper to show a React component in a popup
    const showPopup = (coordinates: [number, number], content: React.ReactNode) => {
        if (!map.current) return;

        // Clean up previous popup
        if (popupRoot.current) {
            popupRoot.current.unmount();
            popupRoot.current = null;
        }
        if (popup.current) {
            popup.current.remove();
        }

        // Create container and render React component
        const container = document.createElement("div");
        container.className = "map-popup";
        popupRoot.current = createRoot(container);
        popupRoot.current.render(content);

        // Create and show popup
        popup.current = new maplibregl.Popup({ closeButton: true, closeOnClick: true, maxWidth: "none" })
            .setLngLat(coordinates)
            .setDOMContent(container)
            .addTo(map.current);

        // Clean up React root when popup closes
        popup.current.on("close", () => {
            if (popupRoot.current) {
                popupRoot.current.unmount();
                popupRoot.current = null;
            }
        });
    };

    useEffect(() => {
        if (!mapContainer.current || map.current) return;

        map.current = new maplibregl.Map({
            container: mapContainer.current,
            style: MAP_STYLE_URL,
            center: [10.898, 48.371],
            zoom: 12,
            pitch: 30,
        });

        // Handle map errors (e.g., style loading failures)
        map.current.on("error", (e) => {
            console.error("Map error:", e.error?.message || e);
        });

        map.current.addControl(new maplibregl.NavigationControl(), "top-right");
        map.current.addControl(new maplibregl.ScaleControl(), "bottom-left");

        map.current.on("load", () => {
            if (!map.current) return;

            // Add 3D buildings
            map.current.addLayer({
                id: "3d-buildings",
                source: "openmaptiles",
                "source-layer": "building",
                type: "fill-extrusion",
                minzoom: 12,
                paint: {
                    "fill-extrusion-color": "#aaa",
                    "fill-extrusion-height": [
                        "interpolate",
                        ["linear"],
                        ["zoom"],
                        12,
                        0,
                        13,
                        ["get", "render_height"],
                    ],
                    "fill-extrusion-base": [
                        "interpolate",
                        ["linear"],
                        ["zoom"],
                        12,
                        0,
                        13,
                        ["get", "render_min_height"],
                    ],
                    "fill-extrusion-opacity": 0.6,
                },
            });

            // Add area outlines source
            map.current.addSource("area-outlines", {
                type: "geojson",
                data: { type: "FeatureCollection", features: [] },
            });

            // Add area fill layer
            map.current.addLayer({
                id: "area-fill",
                type: "fill",
                source: "area-outlines",
                paint: {
                    "fill-color": "#3b82f6",
                    "fill-opacity": 0.1,
                },
            });

            // Add area outline layer
            map.current.addLayer({
                id: "area-outline",
                type: "line",
                source: "area-outlines",
                paint: {
                    "line-color": "#3b82f6",
                    "line-width": 2,
                    "line-dasharray": [2, 2],
                },
            });

            // Add area labels
            map.current.addLayer({
                id: "area-labels",
                type: "symbol",
                source: "area-outlines",
                layout: {
                    "text-field": ["get", "name"],
                    "text-font": ["Open Sans Regular"],
                    "text-size": 14,
                    "text-anchor": "center",
                },
                paint: {
                    "text-color": "#1e40af",
                    "text-halo-color": "#ffffff",
                    "text-halo-width": 2,
                },
            });

            // Add routes source
            map.current.addSource("routes", {
                type: "geojson",
                data: { type: "FeatureCollection", features: [] },
            });

            // Add routes layer (colored lines for each route)
            map.current.addLayer(
                {
                    id: "routes-line",
                    type: "line",
                    source: "routes",
                    paint: {
                        "line-color": ["coalesce", ["get", "color"], "#888888"],
                        "line-width": 4,
                        "line-opacity": 0.8,
                    },
                    layout: {
                        "line-cap": "round",
                        "line-join": "round",
                    },
                },
                "3d-buildings" // Add below 3D buildings
            );

            // Add platform connections source (lines from station to platforms)
            map.current.addSource("platform-connections", {
                type: "geojson",
                data: { type: "FeatureCollection", features: [] },
            });

            // Add platform connections layer (thin gray lines)
            map.current.addLayer({
                id: "platform-connections-line",
                type: "line",
                source: "platform-connections",
                paint: {
                    "line-color": "#888",
                    "line-width": 1,
                    "line-opacity": 0.5,
                },
            });

            // Add platforms source
            map.current.addSource("platforms", {
                type: "geojson",
                data: { type: "FeatureCollection", features: [] },
            });

            // Add platform circles (smaller than stations)
            map.current.addLayer({
                id: "platforms-circle",
                type: "circle",
                source: "platforms",
                paint: {
                    "circle-radius": 5,
                    "circle-color": "#666",
                    "circle-stroke-width": 1,
                    "circle-stroke-color": "#ffffff",
                },
            });

            // Add platform labels (only visible when zoomed in)
            map.current.addLayer({
                id: "platforms-label",
                type: "symbol",
                source: "platforms",
                minzoom: 16,
                layout: {
                    "text-field": ["get", "name"],
                    "text-font": ["Open Sans Regular"],
                    "text-size": 10,
                    "text-offset": [0, 0.9],
                    "text-anchor": "top",
                },
                paint: {
                    "text-color": "#333",
                    "text-halo-color": "#ffffff",
                    "text-halo-width": 1.5,
                },
            });

            // Add stations source
            map.current.addSource("stations", {
                type: "geojson",
                data: { type: "FeatureCollection", features: [] },
            });

            // Add station circles (slightly larger than platforms)
            map.current.addLayer({
                id: "stations-circle",
                type: "circle",
                source: "stations",
                paint: {
                    "circle-radius": 6,
                    "circle-color": "#525252",
                    "circle-stroke-width": 1.5,
                    "circle-stroke-color": "#ffffff",
                },
            });

            // Add station labels
            map.current.addLayer({
                id: "stations-label",
                type: "symbol",
                source: "stations",
                layout: {
                    "text-field": ["get", "name"],
                    "text-font": ["Open Sans Regular"],
                    "text-size": 12,
                    "text-offset": [0, 1.5],
                    "text-anchor": "top",
                },
                paint: {
                    "text-color": "#065f46",
                    "text-halo-color": "#ffffff",
                    "text-halo-width": 2,
                },
            });

            // Add hover cursor for stations
            map.current.on("mouseenter", "stations-circle", () => {
                if (map.current) map.current.getCanvas().style.cursor = "pointer";
            });

            map.current.on("mouseleave", "stations-circle", () => {
                if (map.current) map.current.getCanvas().style.cursor = "";
            });

            // Add hover cursor for platforms
            map.current.on("mouseenter", "platforms-circle", () => {
                if (map.current) map.current.getCanvas().style.cursor = "pointer";
            });

            map.current.on("mouseleave", "platforms-circle", () => {
                if (map.current) map.current.getCanvas().style.cursor = "";
            });

            // Add click popup for stations
            map.current.on("click", "stations-circle", (e) => {
                if (!e.features || e.features.length === 0) return;

                const feature = e.features[0];
                const coordinates = (feature.geometry as GeoJSON.Point).coordinates.slice() as [number, number];
                const osmId = feature.properties?.osm_id;

                // Find the full station object
                const station = stationsRef.current.find((s) => s.osm_id === osmId);
                if (station) {
                    const handlePlatformClick = (platform: StationPlatform | StationStopPosition) => {
                        const platformCoords: [number, number] = [platform.lon, platform.lat];
                        showPopup(platformCoords, <PlatformPopup platform={platform} stationName={station.name ?? undefined} routeColors={routeColorsRef.current} />);
                    };
                    showPopup(coordinates, <StationPopup station={station} onPlatformClick={handlePlatformClick} />);
                }
            });

            // Add click popup for platforms/stop positions
            map.current.on("click", "platforms-circle", (e) => {
                if (!e.features || e.features.length === 0) return;

                const feature = e.features[0];
                const coordinates = (feature.geometry as GeoJSON.Point).coordinates.slice() as [number, number];
                const osmId = feature.properties?.osm_id;
                const stationName = feature.properties?.station_name;

                // Find the platform or stop position object
                for (const station of stationsRef.current) {
                    const platform = station.platforms.find((p) => p.osm_id === osmId);
                    if (platform) {
                        showPopup(coordinates, <PlatformPopup platform={platform} stationName={stationName} routeColors={routeColorsRef.current} />);
                        return;
                    }
                    const stopPosition = station.stop_positions.find((s) => s.osm_id === osmId);
                    if (stopPosition) {
                        showPopup(coordinates, <PlatformPopup platform={stopPosition} stationName={stationName} routeColors={routeColorsRef.current} />);
                        return;
                    }
                }
            });

            // Add vehicles source
            map.current.addSource("vehicles", {
                type: "geojson",
                data: { type: "FeatureCollection", features: [] },
            });

            // Add vehicle markers as a single symbol layer with generated icons
            map.current.addLayer({
                id: "vehicles-marker",
                type: "symbol",
                source: "vehicles",
                layout: {
                    "icon-image": ["get", "iconId"],
                    "icon-size": ICON_SCALE,
                    "icon-allow-overlap": true,
                    "icon-ignore-placement": true,
                },
            });

            // Add hover cursor for vehicles
            map.current.on("mouseenter", "vehicles-marker", () => {
                if (map.current) map.current.getCanvas().style.cursor = "pointer";
            });

            map.current.on("mouseleave", "vehicles-marker", () => {
                if (map.current) map.current.getCanvas().style.cursor = "";
            });

            // Add click popup for vehicles
            map.current.on("click", "vehicles-marker", (e) => {
                if (!e.features || e.features.length === 0) return;

                const feature = e.features[0];
                const coordinates = (feature.geometry as GeoJSON.Point).coordinates.slice() as [number, number];
                const tripId = feature.properties?.tripId;
                const lineNumber = feature.properties?.lineNumber;
                const destination = feature.properties?.destination;
                const status = feature.properties?.status;
                const delayMinutes = feature.properties?.delayMinutes;
                const currentStopName = feature.properties?.currentStopName;
                const nextStopName = feature.properties?.nextStopName;

                showPopup(
                    coordinates,
                    <VehiclePopup
                        tripId={tripId}
                        lineNumber={lineNumber}
                        destination={destination}
                        status={status}
                        delayMinutes={delayMinutes}
                        currentStopName={currentStopName}
                        nextStopName={nextStopName}
                        routeColors={routeColorsRef.current}
                    />
                );
            });

            setMapLoaded(true);
        });

        return () => {
            if (animationRef.current) {
                cancelAnimationFrame(animationRef.current);
                animationRef.current = null;
            }
            if (popupRoot.current) {
                popupRoot.current.unmount();
                popupRoot.current = null;
            }
            popup.current?.remove();
            popup.current = null;
            vehicleIconsRef.current.clear();
            // map.remove() cleans up all event listeners, sources, and layers
            map.current?.remove();
            map.current = null;
        };
    }, []); // Empty deps: map should only initialize once

    // Calculate vehicle positions and update the map
    const updateVehiclePositions = useCallback(() => {
        if (!map.current || !mapLoaded) return;

        const source = map.current.getSource("vehicles") as maplibregl.GeoJSONSource;
        if (!source) return;

        const now = new Date();

        // First, deduplicate vehicles by trip_id - keep the one with most stops
        // This handles cases where the same trip appears on multiple route variants
        const vehiclesByTripId = new globalThis.Map<string, { vehicle: typeof vehiclesRef.current[0]["vehicles"][0]; routeId: number; stopCount: number }>();

        for (const routeVehicles of vehiclesRef.current) {
            for (const vehicle of routeVehicles.vehicles) {
                const existing = vehiclesByTripId.get(vehicle.trip_id);
                if (!existing || vehicle.stops.length > existing.stopCount) {
                    vehiclesByTripId.set(vehicle.trip_id, {
                        vehicle,
                        routeId: routeVehicles.routeId,
                        stopCount: vehicle.stops.length,
                    });
                }
            }
        }

        const features: GeoJSON.Feature[] = [];

        for (const { vehicle, routeId } of vehiclesByTripId.values()) {
            const routeGeometry = routeGeometriesRef.current.get(routeId);
            const routeColor = routeColorsRef.current.get(vehicle.line_number ?? "");

            const position = calculateVehiclePosition(
                vehicle,
                routeGeometry ?? [],
                now
            );

            if (position && position.status !== "completed") {
                const color = routeColor ?? "#3b82f6";
                const lineNum = position.lineNumber ?? "?";
                const iconId = `vehicle-${color.replace("#", "")}-${lineNum}`;

                // Create icon for this color+lineNumber combo if it doesn't exist
                if (!vehicleIconsRef.current.has(iconId) && map.current) {
                    const iconData = createVehicleIcon(color, lineNum);
                    map.current.addImage(iconId, iconData);
                    vehicleIconsRef.current.add(iconId);
                }

                features.push({
                    type: "Feature",
                    properties: {
                        tripId: position.tripId,
                        lineNumber: position.lineNumber,
                        destination: position.destination,
                        status: position.status,
                        delayMinutes: position.delayMinutes,
                        bearing: position.bearing,
                        color,
                        iconId,
                        currentStopName: position.currentStop?.stop_name ?? null,
                        nextStopName: position.nextStop?.stop_name ?? null,
                    },
                    geometry: {
                        type: "Point",
                        coordinates: [position.lon, position.lat],
                    },
                });
            }
        }

        source.setData({ type: "FeatureCollection", features });
    }, [mapLoaded]);

    // Animation loop for smooth vehicle movement
    useEffect(() => {
        if (!mapLoaded || !showVehicles) {
            // Clear vehicles when hidden
            if (map.current && mapLoaded) {
                const source = map.current.getSource("vehicles") as maplibregl.GeoJSONSource;
                if (source) {
                    source.setData({ type: "FeatureCollection", features: [] });
                }
            }
            return;
        }

        const animate = (timestamp: number) => {
            // Only update at the specified interval
            if (timestamp - lastAnimationTimeRef.current >= ANIMATION_INTERVAL) {
                lastAnimationTimeRef.current = timestamp;
                updateVehiclePositions();
            }
            animationRef.current = requestAnimationFrame(animate);
        };

        // Initial update
        updateVehiclePositions();

        // Start animation loop
        animationRef.current = requestAnimationFrame(animate);

        return () => {
            if (animationRef.current) {
                cancelAnimationFrame(animationRef.current);
                animationRef.current = null;
            }
        };
    }, [mapLoaded, showVehicles, updateVehiclePositions]);

    // Also update when vehicles data changes
    useEffect(() => {
        if (mapLoaded && showVehicles) {
            updateVehiclePositions();
        }
    }, [vehicles, mapLoaded, showVehicles, updateVehiclePositions]);

    // Update area outlines when areas or visibility changes
    useEffect(() => {
        if (!map.current || !mapLoaded) return;

        const source = map.current.getSource("area-outlines") as maplibregl.GeoJSONSource;
        if (!source) return;

        if (!showAreaOutlines) {
            source.setData({ type: "FeatureCollection", features: [] });
            return;
        }

        const features = areas.map((area) => ({
            type: "Feature" as const,
            properties: { name: area.name, id: area.id },
            geometry: {
                type: "Polygon" as const,
                coordinates: [
                    [
                        [area.west, area.south],
                        [area.east, area.south],
                        [area.east, area.north],
                        [area.west, area.north],
                        [area.west, area.south],
                    ],
                ],
            },
        }));

        source.setData({ type: "FeatureCollection", features });
    }, [areas, showAreaOutlines, mapLoaded]);

    // Update stations, platforms, and connections when data or visibility changes
    useEffect(() => {
        if (!map.current || !mapLoaded) return;

        const stationSource = map.current.getSource("stations") as maplibregl.GeoJSONSource;
        const platformSource = map.current.getSource("platforms") as maplibregl.GeoJSONSource;
        const connectionSource = map.current.getSource("platform-connections") as maplibregl.GeoJSONSource;
        if (!stationSource || !platformSource || !connectionSource) return;

        if (!showStations) {
            stationSource.setData({ type: "FeatureCollection", features: [] });
            platformSource.setData({ type: "FeatureCollection", features: [] });
            connectionSource.setData({ type: "FeatureCollection", features: [] });
            return;
        }

        // Create station features
        const stationFeatures = stations.map((station) => ({
            type: "Feature" as const,
            properties: { name: station.name, osm_id: station.osm_id },
            geometry: {
                type: "Point" as const,
                coordinates: [station.lon, station.lat],
            },
        }));

        // Create platform features and connection lines
        const platformFeatures: GeoJSON.Feature[] = [];
        const connectionFeatures: GeoJSON.Feature[] = [];

        for (const station of stations) {
            const stationCoord: [number, number] = [station.lon, station.lat];

            // Helper to add a platform/stop position feature
            const addPlatformFeature = (item: StationPlatform | StationStopPosition) => {
                const coord: [number, number] = [item.lon, item.lat];
                const displayName = getPlatformDisplayName(item);

                platformFeatures.push({
                    type: "Feature",
                    properties: {
                        name: displayName,
                        station_name: station.name,
                        osm_id: item.osm_id,
                        ref_ifopt: item.ref_ifopt,
                    },
                    geometry: {
                        type: "Point",
                        coordinates: coord,
                    },
                });

                // Add connection line from station to platform
                connectionFeatures.push({
                    type: "Feature",
                    properties: { station_id: station.osm_id },
                    geometry: {
                        type: "LineString",
                        coordinates: [stationCoord, coord],
                    },
                });
            };

            // Add platforms first (they take precedence), deduplicating by display name
            const addedNames = new Set<string>();
            for (const platform of station.platforms) {
                const name = getPlatformDisplayName(platform);
                if (!addedNames.has(name)) {
                    addedNames.add(name);
                    addPlatformFeature(platform);
                }
            }

            // Add stop positions only if no platform with same name exists
            for (const stopPosition of station.stop_positions) {
                const name = getPlatformDisplayName(stopPosition);
                if (!addedNames.has(name)) {
                    addedNames.add(name);
                    addPlatformFeature(stopPosition);
                }
            }
        }

        stationSource.setData({ type: "FeatureCollection", features: stationFeatures });
        platformSource.setData({ type: "FeatureCollection", features: platformFeatures });
        connectionSource.setData({ type: "FeatureCollection", features: connectionFeatures });
    }, [stations, showStations, mapLoaded]);

    // Update routes when routes or visibility changes
    useEffect(() => {
        if (!map.current || !mapLoaded) return;

        const source = map.current.getSource("routes") as maplibregl.GeoJSONSource;
        if (!source) return;

        if (!showRoutes) {
            source.setData({ type: "FeatureCollection", features: [] });
            return;
        }

        // Create features for each route's geometry segments
        const features: GeoJSON.Feature[] = [];

        for (const route of routes) {
            if (!route.geometry?.segments) continue;

            // Each route can have multiple segments
            for (const segment of route.geometry.segments) {
                if (segment.length < 2) continue;

                features.push({
                    type: "Feature",
                    properties: {
                        route_id: route.osm_id,
                        name: route.name,
                        ref: route.ref,
                        color: route.color || "#888888",
                    },
                    geometry: {
                        type: "LineString",
                        coordinates: segment,
                    },
                });
            }
        }

        source.setData({ type: "FeatureCollection", features });
    }, [routes, showRoutes, mapLoaded]);

    return <div ref={mapContainer} className="w-full h-full" />;
}
