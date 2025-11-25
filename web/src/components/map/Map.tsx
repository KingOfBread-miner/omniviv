import maplibregl from "maplibre-gl";
import "maplibre-gl/dist/maplibre-gl.css";
import React from "react";
import { PopupManager } from "./popupManager";
import { VehiclePosition, VehiclePositionsResponse, Station } from "./types";

interface MapProps {
    className?: string;
}

export default class TramMap extends React.Component<MapProps> {
    private mapContainer: React.RefObject<HTMLDivElement>;
    private map: maplibregl.Map | null = null;
    private vehicleInterval?: number;
    private popupManager?: PopupManager;

    constructor(props: MapProps) {
        super(props);
        this.mapContainer = React.createRef();
    }

    componentDidMount() {
        if (!this.mapContainer.current) return;

        this.initializeMap();
    }

    componentWillUnmount() {
        // Cleanup intervals
        if (this.vehicleInterval) {
            clearInterval(this.vehicleInterval);
        }
        if (this.map) {
            this.map.remove();
            this.map = null;
        }
    }

    private initializeMap = () => {
        if (!this.mapContainer.current) return;

        // Initialize map
        this.map = new maplibregl.Map({
            container: this.mapContainer.current,
            style: "http://localhost:8080/styles/basic-preview/style.json",
            center: [10.898, 48.371], // Augsburg coordinates
            zoom: 16,
            pitch: 30, // Tilt the map for 3D effect
            bearing: 0 // Initial rotation
        });

        // Add navigation controls
        this.map.addControl(new maplibregl.NavigationControl(), "top-right");

        // Add scale control
        this.map.addControl(new maplibregl.ScaleControl(), "bottom-left");

        // Load tram data when map is ready
        this.map.on("load", this.onMapLoad);
    };

    private onMapLoad = async () => {
        if (!this.map) return;

        // Add 3D buildings
        this.map.addLayer({
            id: "3d-buildings",
            source: "openmaptiles",
            "source-layer": "building",
            type: "fill-extrusion",
            minzoom: 14,
            paint: {
                "fill-extrusion-color": "#aaa",
                "fill-extrusion-height": [
                    "interpolate",
                    ["linear"],
                    ["zoom"],
                    15,
                    0,
                    15.05,
                    ["get", "render_height"]
                ],
                "fill-extrusion-base": [
                    "interpolate",
                    ["linear"],
                    ["zoom"],
                    15,
                    0,
                    15.05,
                    ["get", "render_min_height"]
                ],
                "fill-extrusion-opacity": 0.6
            }
        });

        await this.loadTramLines();
        await this.loadTramStations();
        await this.loadVehicles();
    };

    private loadTramLines = async () => {
        if (!this.map) return;

        // Fetch line geometries from the API endpoint
        const lineRefs = ["1", "2", "3", "4", "6"]; // Only include lines 1-6

        try {
            const lineGeometriesResponse = await fetch(
                "http://localhost:3000/api/lines/geometries",
                {
                    method: "POST",
                    headers: {
                        "Content-Type": "application/json"
                    },
                    body: JSON.stringify({ line_refs: lineRefs })
                }
            );

            const lineGeometries = await lineGeometriesResponse.json();
            console.log(`Received ${lineGeometries.length} line geometries from API`);

            // Add each tram line as a separate layer with its own color
            lineGeometries.forEach((lineData: any) => {
                if (lineData.segments.length > 0) {
                    // Add source for this line with multiple line strings
                    this.map!.addSource(`tram-line-${lineData.line_ref}`, {
                        type: "geojson",
                        data: {
                            type: "Feature",
                            properties: {
                                ref: lineData.line_ref,
                                color: lineData.color
                            },
                            geometry: {
                                type: "MultiLineString",
                                coordinates: lineData.segments
                            }
                        }
                    });

                    // Add layer for this line
                    this.map!.addLayer(
                        {
                            id: `tram-line-${lineData.line_ref}`,
                            type: "line",
                            source: `tram-line-${lineData.line_ref}`,
                            paint: {
                                "line-color": lineData.color,
                                "line-width": 4,
                                "line-opacity": 0.8
                            },
                            layout: {
                                "line-cap": "round",
                                "line-join": "round"
                            }
                        },
                        "3d-buildings"
                    );

                    const totalPoints = lineData.segments.reduce(
                        (sum: number, seg: any) => sum + seg.length,
                        0
                    );
                    console.log(
                        `Added line ${lineData.line_ref} with ${lineData.segments.length} segments and ${totalPoints} points in color ${lineData.color}`
                    );
                }
            });
        } catch (error) {
            console.error("Error fetching line geometries:", error);
        }
    };

    private loadTramStations = async () => {
        if (!this.map) return;

        try {
            const stationsResponse = await fetch("http://localhost:3000/api/stations");
            const stationsMap = await stationsResponse.json();
            const stationCount = Object.keys(stationsMap).length;
            console.log(`Received ${stationCount} tram stations with platforms from API`);

            // Initialize popup manager
            this.popupManager = new PopupManager(this.map, stationsMap);

            // Create features for stations and platforms
            const stationFeatures: any[] = [];
            const platformFeatures: any[] = [];
            const connectionLines: any[] = [];

            Object.entries(stationsMap).forEach(([stationId, station]: [string, any]) => {
                // Add station center point (using station coord or average of platform coords)
                let stationCoord: [number, number];

                if (station.coord && station.coord.length === 2) {
                    // EFA API returns [lat, lon] but MapLibre expects [lon, lat]
                    stationCoord = [station.coord[1], station.coord[0]];
                } else if (station.platforms.length > 0) {
                    // Calculate average position from platforms
                    let avgLon = 0;
                    let avgLat = 0;
                    let count = 0;

                    station.platforms.forEach((platform: any) => {
                        if (platform.coord && platform.coord.length === 2) {
                            avgLon += platform.coord[1];
                            avgLat += platform.coord[0];
                            count++;
                        }
                    });

                    if (count > 0) {
                        stationCoord = [avgLon / count, avgLat / count];
                    } else {
                        return; // Skip this station if no coordinates
                    }
                } else {
                    return; // Skip this station if no coordinates
                }

                // Add station feature
                stationFeatures.push({
                    type: "Feature",
                    geometry: {
                        type: "Point",
                        coordinates: stationCoord
                    },
                    properties: {
                        station_id: stationId,
                        station_name: station.station_name
                    }
                });

                // Add platform features and connection lines
                station.platforms.forEach((platform: any) => {
                    if (platform.coord && platform.coord.length === 2) {
                        // EFA API returns [lat, lon] but MapLibre expects [lon, lat]
                        const platformCoord = [platform.coord[1], platform.coord[0]];

                        platformFeatures.push({
                            type: "Feature",
                            geometry: {
                                type: "Point",
                                coordinates: platformCoord
                            },
                            properties: {
                                station_id: stationId,
                                station_name: station.station_name,
                                platform_id: platform.id,
                                platform_name: platform.name,
                                osm_id: platform.osm_id
                            }
                        });

                        // Add line connecting platform to station
                        connectionLines.push({
                            type: "Feature",
                            geometry: {
                                type: "LineString",
                                coordinates: [stationCoord, platformCoord]
                            },
                            properties: {
                                station_id: stationId
                            }
                        });
                    }
                });
            });

            console.log(
                `Created ${stationFeatures.length} station markers and ${platformFeatures.length} platform markers`
            );

            // Add connection lines source and layer
            this.map.addSource("platform-connections", {
                type: "geojson",
                data: {
                    type: "FeatureCollection",
                    features: connectionLines
                }
            });

            this.map.addLayer({
                id: "platform-connections-line",
                type: "line",
                source: "platform-connections",
                paint: {
                    "line-color": "#888",
                    "line-width": 1,
                    "line-opacity": 0.5
                }
            });

            // Add station points source
            this.map.addSource("tram-stations", {
                type: "geojson",
                data: {
                    type: "FeatureCollection",
                    features: stationFeatures
                }
            });

            // Add platform points source
            this.map.addSource("tram-platforms", {
                type: "geojson",
                data: {
                    type: "FeatureCollection",
                    features: platformFeatures
                }
            });

            // Add platform circles (smaller)
            this.map.addLayer({
                id: "tram-platforms-circle",
                type: "circle",
                source: "tram-platforms",
                paint: {
                    "circle-radius": 5,
                    "circle-color": "#666",
                    "circle-stroke-width": 1,
                    "circle-stroke-color": "#ffffff"
                }
            });

            // Add station circles (bigger)
            this.map.addLayer({
                id: "tram-stations-circle",
                type: "circle",
                source: "tram-stations",
                paint: {
                    "circle-radius": 8,
                    "circle-color": "#444",
                    "circle-stroke-width": 2,
                    "circle-stroke-color": "#ffffff"
                }
            });

            // Add station labels (show station name)
            this.map.addLayer({
                id: "tram-stations-label",
                type: "symbol",
                source: "tram-stations",
                layout: {
                    "text-field": ["get", "station_name"],
                    "text-font": ["Open Sans Regular"],
                    "text-offset": [0, 1.2],
                    "text-anchor": "top",
                    "text-size": 12,
                    "symbol-sort-key": 0 // Give station labels highest priority
                },
                paint: {
                    "text-color": "#000000",
                    "text-halo-color": "#ffffff",
                    "text-halo-width": 2
                }
            });

            // Add platform labels (show platform name, only visible when zoomed in)
            this.map.addLayer({
                id: "tram-platforms-label",
                type: "symbol",
                source: "tram-platforms",
                minzoom: 17, // Only show platform labels when zoomed in
                layout: {
                    "text-field": ["get", "platform_name"],
                    "text-font": ["Open Sans Regular"],
                    "text-offset": [0, 0.9],
                    "text-anchor": "top",
                    "text-size": 10,
                    "symbol-sort-key": 1 // Lower priority than station labels
                },
                paint: {
                    "text-color": "#333",
                    "text-halo-color": "#ffffff",
                    "text-halo-width": 1.5
                }
            });

            // Add unified click handler for stations and platforms
            this.map.on("click", this.handleMapClick);

            // Change cursor on hover for stations
            this.map.on("mouseenter", "tram-stations-circle", () => {
                if (this.map) this.map.getCanvas().style.cursor = "pointer";
            });

            this.map.on("mouseleave", "tram-stations-circle", () => {
                if (this.map) this.map.getCanvas().style.cursor = "";
            });

            // Change cursor on hover for platforms
            this.map.on("mouseenter", "tram-platforms-circle", () => {
                if (this.map) this.map.getCanvas().style.cursor = "pointer";
            });

            this.map.on("mouseleave", "tram-platforms-circle", () => {
                if (this.map) this.map.getCanvas().style.cursor = "";
            });
        } catch (error) {
            console.error("Error loading tram stations:", error);
        }
    };

    private handleMapClick = (e: maplibregl.MapMouseEvent) => {
        if (!this.map || !this.popupManager) return;

        // Query all features at the click point
        const features = this.map.queryRenderedFeatures(e.point, {
            layers: ["tram-stations-circle", "tram-platforms-circle"]
        });

        if (!features || features.length === 0) return;

        // Check if a station was clicked (stations have priority)
        const stationFeature = features.find((f) => f.layer.id === "tram-stations-circle");

        if (stationFeature) {
            // Show station popup
            const coordinates = (stationFeature.geometry as any).coordinates.slice();
            const stationName = stationFeature.properties?.station_name;
            const stationId = stationFeature.properties?.station_id;

            this.popupManager.showStationPopup(stationId, stationName, coordinates);
            return;
        }

        // If no station, check for platform
        const platformFeature = features.find((f) => f.layer.id === "tram-platforms-circle");

        if (platformFeature) {
            // Show platform popup
            const coordinates = (platformFeature.geometry as any).coordinates.slice();
            const stationId = platformFeature.properties?.station_id;
            const stationName = platformFeature.properties?.station_name;
            const platformName = platformFeature.properties?.platform_name;
            const platformId = platformFeature.properties?.platform_id;
            const osmId = platformFeature.properties?.osm_id;

            this.popupManager.showPlatformPopupFromMap(
                platformName,
                stationName,
                platformId,
                stationId,
                osmId,
                coordinates
            );
        }
    };

    private loadVehicles = async () => {
        if (!this.map) return;

        // Fetch and display vehicle positions
        const fetchAndUpdateVehicles = async () => {
            try {
                const response = await fetch(
                    "http://localhost:3000/api/vehicles/position_estimates"
                );
                const data: VehiclePositionsResponse = await response.json();

                console.log(`Received positions for ${Object.keys(data.vehicles).length} vehicles`);

                // Convert to GeoJSON features
                const vehicleFeatures = Object.values(data.vehicles).map((vehicle) => ({
                    type: "Feature" as const,
                    geometry: {
                        type: "Point" as const,
                        coordinates: vehicle.coordinates
                    },
                    properties: {
                        vehicle_id: vehicle.vehicle_id,
                        line_number: vehicle.line_number,
                        line_name: vehicle.line_name,
                        destination: vehicle.destination,
                        progress: vehicle.progress,
                        delay: vehicle.delay || 0
                    }
                }));

                // Update vehicle source
                const source = this.map?.getSource("vehicles") as maplibregl.GeoJSONSource;
                if (source) {
                    source.setData({
                        type: "FeatureCollection",
                        features: vehicleFeatures
                    });
                }
            } catch (error) {
                console.error("Error fetching vehicle positions:", error);
            }
        };

        // Initial fetch to populate vehicle data
        await fetchAndUpdateVehicles();

        // Create vehicle source and layers after first fetch
        if (this.map && !this.map.getSource("vehicles")) {
            this.map.addSource("vehicles", {
                type: "geojson",
                data: {
                    type: "FeatureCollection",
                    features: []
                }
            });

            // Add vehicle circle layer
            this.map.addLayer({
                id: "vehicles-circle",
                type: "circle",
                source: "vehicles",
                paint: {
                    "circle-radius": 8,
                    "circle-color": "#FF6B35",
                    "circle-stroke-width": 2,
                    "circle-stroke-color": "#ffffff",
                    "circle-opacity": 0.9
                }
            });

            // Add vehicle label layer
            this.map.addLayer({
                id: "vehicles-label",
                type: "symbol",
                source: "vehicles",
                layout: {
                    "text-field": ["get", "line_number"],
                    "text-font": ["Open Sans Bold"],
                    "text-size": 11,
                    "text-offset": [0, 0],
                    "text-anchor": "center"
                },
                paint: {
                    "text-color": "#ffffff"
                }
            });

            // Add hover cursor
            this.map.on("mouseenter", "vehicles-circle", () => {
                if (this.map) this.map.getCanvas().style.cursor = "pointer";
            });

            this.map.on("mouseleave", "vehicles-circle", () => {
                if (this.map) this.map.getCanvas().style.cursor = "";
            });

            // Add click handler for vehicles
            this.map.on("click", "vehicles-circle", (e) => {
                if (!e.features || e.features.length === 0) return;

                const feature = e.features[0];
                const props = feature.properties;

                const popupHTML = `
                    <div class="bg-white rounded-lg p-3">
                        <div class="flex items-center gap-2 mb-2">
                            <span class="text-lg font-bold text-orange-600">Line ${props?.line_number}</span>
                            <span class="text-sm text-gray-600">→ ${props?.destination}</span>
                        </div>
                        <div class="text-xs text-gray-500 space-y-1">
                            <div>Progress: ${((props?.progress || 0) * 100).toFixed(0)}%</div>
                            ${props?.delay && props.delay > 0 ? `<div class="text-red-600">Delay: +${props.delay} min</div>` : ""}
                        </div>
                    </div>
                `;

                new maplibregl.Popup({ closeButton: false, closeOnClick: true })
                    .setLngLat((feature.geometry as any).coordinates.slice())
                    .setHTML(popupHTML)
                    .addTo(this.map!);
            });

        }

        // Update vehicle data from server every 5 seconds
        this.vehicleInterval = window.setInterval(fetchAndUpdateVehicles, 5000);
    };

    render() {
        return (
            <div
                ref={this.mapContainer}
                className={this.props.className}
                style={{ width: "100%", height: "100%" }}
            />
        );
    }
}
