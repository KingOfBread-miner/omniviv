import maplibregl from "maplibre-gl";
import "maplibre-gl/dist/maplibre-gl.css";
import React from "react";
import { createRoot, type Root } from "react-dom/client";
import type { Area, Station, StationPlatform, StationStopPosition } from "../../api";
import type { RouteVehicles, RouteWithGeometry } from "../../App";
import { PlatformPopup } from "../PlatformPopup";
import { StationPopup } from "../StationPopup";
import { VehicleRenderer } from "../vehicles/VehicleRenderer";
import { VehicleTracker, type TrackingInfo } from "../vehicles/VehicleTracker";
import { MapLayerManager } from "./MapLayerManager";

const MAP_STYLE_URL = import.meta.env.VITE_MAP_STYLE_URL ?? "/styles/basic-preview/style.json";
const ANIMATION_INTERVAL = 50;

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

interface MapState {
    mapLoaded: boolean;
    trackedTripId: string | null;
    trackingInfo: TrackingInfo | null;
}

export default class Map extends React.Component<MapProps, MapState> {
    private mapContainer: React.RefObject<HTMLDivElement | null>;
    private map: maplibregl.Map | null = null;
    private popup: maplibregl.Popup | null = null;
    private popupRoot: Root | null = null;

    // Managers
    private layerManager: MapLayerManager | null = null;
    private vehicleRenderer: VehicleRenderer | null = null;
    private vehicleTracker: VehicleTracker | null = null;

    // Data caches
    private routeColors = new globalThis.Map<string, string>();
    private routeGeometries = new globalThis.Map<number, number[][][]>();

    constructor(props: MapProps) {
        super(props);
        this.mapContainer = React.createRef();
        this.state = {
            mapLoaded: false,
            trackedTripId: null,
            trackingInfo: null,
        };
    }

    componentDidMount() {
        this.initializeMap();
        this.updateRouteData();
    }

    componentDidUpdate(prevProps: MapProps, prevState: MapState) {
        if (prevProps.routes !== this.props.routes) {
            this.updateRouteData();
        }

        if (this.state.mapLoaded && !prevState.mapLoaded) {
            this.updateAllMapData();
        }

        if (this.state.mapLoaded && this.layerManager) {
            if (prevProps.showAreaOutlines !== this.props.showAreaOutlines || prevProps.areas !== this.props.areas) {
                this.layerManager.updateAreaOutlines(this.props.areas, this.props.showAreaOutlines);
            }
            if (prevProps.showStations !== this.props.showStations || prevProps.stations !== this.props.stations) {
                this.layerManager.updateStations(this.props.stations, this.props.showStations);
            }
            if (prevProps.showRoutes !== this.props.showRoutes || prevProps.routes !== this.props.routes) {
                this.layerManager.updateRoutes(this.props.routes, this.props.showRoutes);
            }
            if (prevProps.showVehicles !== this.props.showVehicles) {
                this.handleVehicleVisibilityChange();
            }
            if (prevProps.vehicles !== this.props.vehicles && this.props.showVehicles) {
                this.updateVehicles();
            }
        }

        if (prevState.trackedTripId !== this.state.trackedTripId) {
            this.handleTrackingChange(prevState.trackedTripId);
        }
    }

    componentWillUnmount() {
        this.cleanup();
    }

    private cleanup() {
        this.vehicleRenderer?.dispose();
        this.vehicleTracker?.dispose();

        if (this.popupRoot) {
            this.popupRoot.unmount();
            this.popupRoot = null;
        }
        this.popup?.remove();
        this.popup = null;
        this.map?.remove();
        this.map = null;
    }

    private updateRouteData() {
        const colorMap = new globalThis.Map<string, string>();
        const geometryMap = new globalThis.Map<number, number[][][]>();

        for (const route of this.props.routes) {
            if (route.ref && route.color) {
                colorMap.set(route.ref, route.color);
            }
            if (route.geometry?.segments) {
                geometryMap.set(route.osm_id, route.geometry.segments);
            }
        }

        this.routeColors = colorMap;
        this.routeGeometries = geometryMap;

        this.vehicleRenderer?.updateRouteData(colorMap, geometryMap);
    }

    private updateAllMapData() {
        if (!this.layerManager) return;

        this.layerManager.updateAreaOutlines(this.props.areas, this.props.showAreaOutlines);
        this.layerManager.updateStations(this.props.stations, this.props.showStations);
        this.layerManager.updateRoutes(this.props.routes, this.props.showRoutes);

        if (this.props.showVehicles) {
            this.startVehicleAnimation();
        }
    }

    private showPopup = (coordinates: [number, number], content: React.ReactNode) => {
        if (!this.map) return;

        if (this.popupRoot) {
            this.popupRoot.unmount();
            this.popupRoot = null;
        }
        if (this.popup) {
            this.popup.remove();
        }

        const container = document.createElement("div");
        container.className = "map-popup";
        this.popupRoot = createRoot(container);
        this.popupRoot.render(content);

        this.popup = new maplibregl.Popup({ closeButton: true, closeOnClick: true, maxWidth: "none" })
            .setLngLat(coordinates)
            .setDOMContent(container)
            .addTo(this.map);

        this.popup.on("close", () => {
            if (this.popupRoot) {
                this.popupRoot.unmount();
                this.popupRoot = null;
            }
        });
    };

    private initializeMap() {
        if (!this.mapContainer.current || this.map) return;

        this.map = new maplibregl.Map({
            container: this.mapContainer.current,
            style: MAP_STYLE_URL,
            center: [10.898, 48.371],
            zoom: 12,
            pitch: 30,
        });

        this.map.on("error", (e) => {
            console.error("Map error:", e.error?.message || e);
        });

        this.map.addControl(new maplibregl.NavigationControl(), "top-right");
        this.map.addControl(new maplibregl.ScaleControl(), "bottom-left");

        this.map.on("load", () => {
            if (!this.map) return;

            // Initialize managers
            this.layerManager = new MapLayerManager(this.map);
            this.layerManager.setupLayers();

            this.vehicleRenderer = new VehicleRenderer(this.layerManager, this.routeColors, this.routeGeometries);
            this.vehicleRenderer.setOnTrackedVehicleLost(() => {
                this.setState({ trackedTripId: null });
            });

            this.vehicleTracker = new VehicleTracker(this.map, {
                onTrackingInfoUpdate: (info) => this.setState({ trackingInfo: info }),
                onTrackingStop: () => this.setState({ trackedTripId: null }),
                getSmoothedPosition: (tripId) => this.vehicleRenderer?.getSmoothedPosition(tripId),
                getRouteColor: (lineNumber) => this.routeColors.get(lineNumber) ?? "#3b82f6",
            });

            this.setupMapEventHandlers();
            this.setState({ mapLoaded: true });
        });
    }

    private setupMapEventHandlers() {
        if (!this.map) return;

        // Hover cursors
        this.map.on("mouseenter", "stations-circle", () => { if (this.map) this.map.getCanvas().style.cursor = "pointer"; });
        this.map.on("mouseleave", "stations-circle", () => { if (this.map) this.map.getCanvas().style.cursor = ""; });
        this.map.on("mouseenter", "platforms-circle", () => { if (this.map) this.map.getCanvas().style.cursor = "pointer"; });
        this.map.on("mouseleave", "platforms-circle", () => { if (this.map) this.map.getCanvas().style.cursor = ""; });
        this.map.on("mouseenter", "vehicles-marker", () => { if (this.map) this.map.getCanvas().style.cursor = "pointer"; });
        this.map.on("mouseleave", "vehicles-marker", () => { if (this.map) this.map.getCanvas().style.cursor = ""; });

        // Station click
        this.map.on("click", "stations-circle", (e) => {
            if (!e.features || e.features.length === 0) return;
            const feature = e.features[0];
            const coordinates = (feature.geometry as GeoJSON.Point).coordinates.slice() as [number, number];
            const osmId = feature.properties?.osm_id;
            const station = this.props.stations.find((s) => s.osm_id === osmId);
            if (station) {
                const handlePlatformClick = (platform: StationPlatform | StationStopPosition) => {
                    const platformCoords: [number, number] = [platform.lon, platform.lat];
                    this.showPopup(platformCoords, <PlatformPopup platform={platform} stationName={station.name ?? undefined} routeColors={this.routeColors} />);
                };
                this.showPopup(coordinates, <StationPopup station={station} onPlatformClick={handlePlatformClick} />);
            }
        });

        // Platform click
        this.map.on("click", "platforms-circle", (e) => {
            if (!e.features || e.features.length === 0) return;
            const feature = e.features[0];
            const coordinates = (feature.geometry as GeoJSON.Point).coordinates.slice() as [number, number];
            const osmId = feature.properties?.osm_id;
            const stationName = feature.properties?.station_name;
            for (const station of this.props.stations) {
                const platform = station.platforms.find((p) => p.osm_id === osmId);
                if (platform) {
                    this.showPopup(coordinates, <PlatformPopup platform={platform} stationName={stationName} routeColors={this.routeColors} />);
                    return;
                }
                const stopPosition = station.stop_positions.find((s) => s.osm_id === osmId);
                if (stopPosition) {
                    this.showPopup(coordinates, <PlatformPopup platform={stopPosition} stationName={stationName} routeColors={this.routeColors} />);
                    return;
                }
            }
        });

        // Vehicle click - toggle tracking
        this.map.on("click", "vehicles-marker", (e) => {
            if (!e.features || e.features.length === 0) return;
            const tripId = e.features[0].properties?.tripId;
            this.setState((state) => ({ trackedTripId: state.trackedTripId === tripId ? null : tripId }));
        });

        // Map click - stop tracking
        this.map.on("click", (e) => {
            const features = this.map?.queryRenderedFeatures(e.point, { layers: ["vehicles-marker"] });
            if (!features || features.length === 0) {
                this.setState({ trackedTripId: null });
            }
        });
    }

    private handleVehicleVisibilityChange() {
        if (this.props.showVehicles) {
            this.startVehicleAnimation();
        } else {
            this.vehicleRenderer?.clear();
        }
    }

    private startVehicleAnimation() {
        if (!this.vehicleRenderer) return;
        this.vehicleRenderer.startAnimation(this.props.vehicles);
    }

    private updateVehicles() {
        if (!this.vehicleRenderer) return;

        this.vehicleRenderer.setTrackedTripId(this.state.trackedTripId);
        this.vehicleRenderer.updatePositions(this.props.vehicles, ANIMATION_INTERVAL);
    }

    private handleTrackingChange(prevTrackedTripId: string | null) {
        if (prevTrackedTripId && !this.state.trackedTripId) {
            // Stopped tracking
            this.vehicleTracker?.stopTracking();
            this.setState({ trackingInfo: null });
        } else if (this.state.trackedTripId && this.vehicleTracker) {
            // Started tracking
            this.vehicleTracker.startTracking(this.state.trackedTripId);
        }
    }

    render() {
        const { trackingInfo } = this.state;

        return (
            <div className="relative w-full h-full">
                <div ref={this.mapContainer} className="w-full h-full" />
                {trackingInfo && (
                    <div className="absolute left-1/2 top-1/2 -translate-x-1/2 -translate-y-[calc(100%+50px)] pointer-events-none">
                        <div className="bg-white px-4 py-3 rounded-lg shadow-lg text-sm text-gray-800 min-w-48">
                            <div className="font-bold text-base mb-1">
                                {trackingInfo.lineNumber} → {trackingInfo.destination}
                            </div>
                            {trackingInfo.nextStopName && (
                                <div className="text-gray-600">
                                    <span className="font-medium">Next:</span> {trackingInfo.nextStopName}
                                </div>
                            )}
                            <div className="flex items-center gap-2 mt-2">
                                <div className="flex-1 h-2 bg-gray-200 rounded-full overflow-hidden">
                                    <div
                                        className="h-full transition-all duration-300"
                                        style={{
                                            width: `${Math.round(trackingInfo.progress * 100)}%`,
                                            backgroundColor: trackingInfo.color,
                                        }}
                                    />
                                </div>
                                {trackingInfo.secondsToNextStop !== null && (
                                    <span className="text-xs text-gray-500 font-mono tabular-nums">
                                        {`${Math.floor(trackingInfo.secondsToNextStop / 60)}m ${String(trackingInfo.secondsToNextStop % 60).padStart(2, "0")}s`}
                                    </span>
                                )}
                            </div>
                        </div>
                    </div>
                )}
            </div>
        );
    }
}
