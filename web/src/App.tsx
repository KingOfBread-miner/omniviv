import { useCallback, useEffect, useMemo, useState } from "react";
import { AlertTriangle, Bug, Clock, Layers, Moon, Navigation, Sun, Wifi, WifiOff } from "lucide-react";
import { Api, Area, Route, RouteGeometry, Station, Vehicle } from "./api";
import { OsmIssuesPanel } from "./components/IssuesPanel";
import { NavigationPanel } from "./components/NavigationPanel";
import { TimeControlPanel } from "./components/TimeControlPanel";
import { Button } from "./components/ui/button";
import { Checkbox } from "./components/ui/checkbox";
import Map from "./components/map/Map";
import type { DebugOptions } from "./components/vehicles/VehicleRenderer";
import { useTimeSimulation } from "./hooks/useTimeSimulation";
import { useVehicleUpdates, type RouteVehicles } from "./hooks/useVehicleUpdates";

type SidebarPanel = "navigation" | "layers" | "debug" | "issues" | "time" | null;

const API_URL = import.meta.env.VITE_API_URL ?? "http://localhost:3000";
const api = new Api({ baseUrl: API_URL });

// Fallback polling interval when WebSocket is not available (in milliseconds)
const FALLBACK_REFRESH_INTERVAL = 5000;

// LocalStorage key for persisted options
const STORAGE_KEY = "live-tram-options";

export interface RouteWithGeometry extends Route {
    geometry: RouteGeometry | null;
}

// Re-export for use by other components
export type { RouteVehicles } from "./hooks/useVehicleUpdates";

// Local type alias for state
type RouteVehiclesData = RouteVehicles;

interface PersistedOptions {
    showAreaOutlines: boolean;
    showStations: boolean;
    showStopPositions: boolean;
    showPlatforms: boolean;
    showRoutes: boolean;
    showVehicles: boolean;
    debugOptions: DebugOptions;
}

const DEFAULT_OPTIONS: PersistedOptions = {
    showAreaOutlines: false,
    showStations: true,
    showStopPositions: false,
    showPlatforms: false,
    showRoutes: true,
    showVehicles: true,
    debugOptions: {
        show3DModels: true,
        showDebugSegments: false,
        showDebugOnlyTracked: true,
    },
};

function loadOptions(): PersistedOptions {
    try {
        const stored = localStorage.getItem(STORAGE_KEY);
        if (stored) {
            const parsed = JSON.parse(stored);
            // Merge with defaults to handle new options added in future versions
            return {
                ...DEFAULT_OPTIONS,
                ...parsed,
                debugOptions: {
                    ...DEFAULT_OPTIONS.debugOptions,
                    ...(parsed.debugOptions || {}),
                },
            };
        }
    } catch (e) {
        console.error("Failed to load options from localStorage:", e);
    }
    return DEFAULT_OPTIONS;
}

function saveOptions(options: PersistedOptions): void {
    try {
        localStorage.setItem(STORAGE_KEY, JSON.stringify(options));
    } catch (e) {
        console.error("Failed to save options to localStorage:", e);
    }
}

export default function App() {
    const [areas, setAreas] = useState<Area[]>([]);
    const [stations, setStations] = useState<Station[]>([]);
    const [routes, setRoutes] = useState<RouteWithGeometry[]>([]);
    const [vehicles, setVehicles] = useState<RouteVehiclesData[]>([]);
    const [activePanel, setActivePanel] = useState<SidebarPanel>(null);
    const [osmIssuesCount, setOsmIssuesCount] = useState<number | null>(null);

    // Theme state
    const [isDark, setIsDark] = useState(() => {
        const stored = localStorage.getItem("theme");
        if (stored) return stored === "dark";
        return window.matchMedia("(prefers-color-scheme: dark)").matches;
    });

    useEffect(() => {
        document.documentElement.classList.toggle("dark", isDark);
        localStorage.setItem("theme", isDark ? "dark" : "light");
    }, [isDark]);

    // Time simulation
    const timeSimulation = useTimeSimulation();

    // Load persisted options from localStorage
    const [options, setOptions] = useState<PersistedOptions>(loadOptions);

    // Save options to localStorage whenever they change
    useEffect(() => {
        saveOptions(options);
    }, [options]);

    // Destructure for easier access
    const { showAreaOutlines, showStations, showStopPositions, showPlatforms, showRoutes, showVehicles, debugOptions } = options;

    // Memoize vehicle count to avoid recalculating on every render
    const totalVehicleCount = useMemo(
        () => vehicles.reduce((acc, rv) => acc + rv.vehicles.length, 0),
        [vehicles]
    );

    // Helper to update a single option
    const updateOption = <K extends keyof PersistedOptions>(key: K, value: PersistedOptions[K]) => {
        setOptions((prev) => ({ ...prev, [key]: value }));
    };

    // Toggle sidebar panel
    const togglePanel = (panel: SidebarPanel) => {
        setActivePanel((current) => (current === panel ? null : panel));
    };

    // Fetch OSM issues count
    useEffect(() => {
        const fetchIssuesCount = async () => {
            try {
                const response = await fetch(`${API_URL}/api/issues`);
                if (response.ok) {
                    const data = await response.json();
                    setOsmIssuesCount(data.count);
                }
            } catch (error) {
                console.error("Failed to fetch issues count:", error);
            }
        };
        fetchIssuesCount();
    }, []);

    // Fetch vehicles for all routes (used as fallback when WebSocket unavailable)
    const fetchVehiclesFallback = useCallback(async () => {
        if (routes.length === 0) return;

        try {
            const vehiclePromises = routes.map(async (route) => {
                try {
                    const response = await api.api.getVehiclesByRoute({ route_id: route.osm_id });
                    return {
                        routeId: route.osm_id,
                        lineNumber: response.data.line_number ?? null,
                        vehicles: response.data.vehicles,
                    };
                } catch {
                    return {
                        routeId: route.osm_id,
                        lineNumber: route.ref ?? null,
                        vehicles: [],
                    };
                }
            });

            const results = await Promise.all(vehiclePromises);
            setVehicles(results);
        } catch (err) {
            console.error("Failed to fetch vehicles:", err);
        }
    }, [routes]);

    // Handle full vehicle data from WebSocket (initial subscribe)
    const handleFullVehicleData = useCallback((data: RouteVehiclesData[]) => {
        setVehicles(data);
    }, []);

    // Handle incremental updates from WebSocket (only changes)
    const handleIncrementalUpdate = useCallback((updater: (current: RouteVehiclesData[]) => RouteVehiclesData[]) => {
        setVehicles(updater);
    }, []);

    // Initial data fetch
    useEffect(() => {
        const fetchData = async () => {
            try {
                const [areasResponse, stationsResponse, routesResponse] = await Promise.all([
                    api.api.listAreas(),
                    api.api.listStations(),
                    api.api.listRoutes(),
                ]);
                setAreas(areasResponse.data.areas);
                setStations(stationsResponse.data.stations);

                // Fetch geometries for all routes
                const routesWithGeometry = await Promise.all(
                    routesResponse.data.routes.map(async (route) => {
                        try {
                            const geomResponse = await api.api.getRouteGeometry(route.osm_id);
                            return { ...route, geometry: geomResponse.data };
                        } catch {
                            return { ...route, geometry: null };
                        }
                    })
                );
                setRoutes(routesWithGeometry);
                // Vehicle data will be fetched via WebSocket subscription
            } catch (err) {
                console.error("Failed to fetch data:", err);
            }
        };

        fetchData();
    }, []);

    // Get route IDs for WebSocket subscription
    const routeIds = useMemo(() => routes.map(r => r.osm_id), [routes]);

    // WebSocket-based vehicle updates with fallback to polling
    const { isConnected: wsConnected, usingWebSocket } = useVehicleUpdates({
        enabled: showVehicles && routes.length > 0,
        routeIds,
        onFullData: handleFullVehicleData,
        onIncrementalUpdate: handleIncrementalUpdate,
        onFallbackFetch: fetchVehiclesFallback,
        fallbackInterval: FALLBACK_REFRESH_INTERVAL,
    });

    return (
        <div className="h-screen w-screen relative flex">
            {/* Sidebar */}
            <div className="flex h-full z-20">
                {/* Icon bar */}
                <div className="flex flex-col bg-background border-r shadow-lg">
                    <Button
                        variant={activePanel === "navigation" ? "default" : "ghost"}
                        size="icon"
                        onClick={() => togglePanel("navigation")}
                        className="m-2"
                        aria-label="Route Planning"
                    >
                        <Navigation className="h-5 w-5" />
                    </Button>
                    <Button
                        variant={activePanel === "layers" ? "default" : "ghost"}
                        size="icon"
                        onClick={() => togglePanel("layers")}
                        className="m-2"
                        aria-label="Layers"
                    >
                        <Layers className="h-5 w-5" />
                    </Button>
                    <Button
                        variant={activePanel === "debug" ? "default" : "ghost"}
                        size="icon"
                        onClick={() => togglePanel("debug")}
                        className="m-2"
                        aria-label="Debug"
                    >
                        <Bug className="h-5 w-5" />
                    </Button>
                    <Button
                        variant={activePanel === "issues" ? "default" : "ghost"}
                        size="icon"
                        onClick={() => togglePanel("issues")}
                        className="m-2 relative"
                        aria-label="OSM Issues"
                    >
                        <AlertTriangle className="h-5 w-5" />
                        {osmIssuesCount !== null && osmIssuesCount > 0 && (
                            <span
                                className="absolute -top-1 -right-1 bg-orange-500 text-white text-xs rounded-full h-5 min-w-5 flex items-center justify-center px-1"
                                aria-label={`${osmIssuesCount} OSM data issues`}
                            >
                                {osmIssuesCount}
                            </span>
                        )}
                    </Button>
                    <Button
                        variant={activePanel === "time" ? "default" : "ghost"}
                        size="icon"
                        onClick={() => togglePanel("time")}
                        className="m-2 relative"
                        aria-label="Time Control"
                    >
                        <Clock className="h-5 w-5" />
                        {!timeSimulation.isRealTime && (
                            <span className="absolute -top-1 -right-1 bg-orange-500 text-white text-xs rounded-full h-3 w-3" />
                        )}
                    </Button>
                    <div className="flex-1" />
                    <Button
                        variant="ghost"
                        size="icon"
                        onClick={() => setIsDark(!isDark)}
                        className="m-2"
                        aria-label="Toggle theme"
                    >
                        {isDark ? <Sun className="h-5 w-5" /> : <Moon className="h-5 w-5" />}
                    </Button>
                </div>

                {/* Content panel */}
                {activePanel && (
                    <div className="w-80 h-full bg-background border-r shadow-lg overflow-y-auto">
                        {activePanel === "navigation" && (
                            <NavigationPanel stations={stations} />
                        )}

                        {activePanel === "layers" && (
                            <div className="p-4">
                                <h2 className="font-semibold mb-4">Layers</h2>
                                <div className="space-y-3">
                                    <label className="flex items-center gap-3 cursor-pointer">
                                        <Checkbox
                                            checked={showAreaOutlines}
                                            onCheckedChange={(checked) => updateOption("showAreaOutlines", checked === true)}
                                        />
                                        <span className="text-sm">Show area outlines</span>
                                    </label>

                                    <label className="flex items-center gap-3 cursor-pointer">
                                        <Checkbox
                                            checked={showStations}
                                            onCheckedChange={(checked) => updateOption("showStations", checked === true)}
                                        />
                                        <span className="text-sm">Show stations ({stations.length})</span>
                                    </label>

                                    <label className="flex items-center gap-3 cursor-pointer pl-6">
                                        <Checkbox
                                            checked={showStopPositions}
                                            onCheckedChange={(checked) => updateOption("showStopPositions", checked === true)}
                                            disabled={!showStations}
                                        />
                                        <span className={`text-sm flex items-center gap-2 ${showStations ? "" : "text-muted-foreground"}`}>
                                            <span className="w-3 h-3 rounded-full bg-blue-500 shrink-0" />
                                            Show stop positions
                                        </span>
                                    </label>

                                    <label className="flex items-center gap-3 cursor-pointer pl-6">
                                        <Checkbox
                                            checked={showPlatforms}
                                            onCheckedChange={(checked) => updateOption("showPlatforms", checked === true)}
                                            disabled={!showStations}
                                        />
                                        <span className={`text-sm flex items-center gap-2 ${showStations ? "" : "text-muted-foreground"}`}>
                                            <span className="w-3 h-3 rounded-full bg-orange-500 shrink-0" />
                                            Show platforms
                                        </span>
                                    </label>

                                    <label className="flex items-center gap-3 cursor-pointer">
                                        <Checkbox
                                            checked={showRoutes}
                                            onCheckedChange={(checked) => updateOption("showRoutes", checked === true)}
                                        />
                                        <span className="text-sm">Show routes ({routes.length})</span>
                                    </label>

                                    <label className="flex items-center gap-3 cursor-pointer">
                                        <Checkbox
                                            checked={showVehicles}
                                            onCheckedChange={(checked) => updateOption("showVehicles", checked === true)}
                                        />
                                        <span className="text-sm flex items-center gap-2">
                                            Show vehicles ({totalVehicleCount})
                                            {showVehicles && (
                                                <span title={usingWebSocket && wsConnected ? "Live updates via WebSocket" : "Polling for updates"}>
                                                    {usingWebSocket && wsConnected ? (
                                                        <Wifi className="h-3 w-3 text-green-500" />
                                                    ) : (
                                                        <WifiOff className="h-3 w-3 text-muted-foreground" />
                                                    )}
                                                </span>
                                            )}
                                        </span>
                                    </label>

                                    <label className="flex items-center gap-3 cursor-pointer pl-6">
                                        <Checkbox
                                            checked={debugOptions.show3DModels}
                                            onCheckedChange={(checked) => updateOption("debugOptions", {
                                                ...debugOptions,
                                                show3DModels: checked === true,
                                            })}
                                            disabled={!showVehicles}
                                        />
                                        <span className={`text-sm ${showVehicles ? "" : "text-muted-foreground"}`}>
                                            Show 3D models
                                        </span>
                                    </label>
                                </div>

                                {areas.length > 0 && (
                                    <div className="mt-6 pt-4 border-t">
                                        <h3 className="text-sm font-medium text-muted-foreground mb-2">Areas</h3>
                                        <ul className="space-y-1">
                                            {areas.map((area) => (
                                                <li key={area.id} className="text-sm flex items-center gap-2">
                                                    <span className="w-2 h-2 rounded-full bg-primary" />
                                                    {area.name}
                                                </li>
                                            ))}
                                        </ul>
                                    </div>
                                )}
                            </div>
                        )}

                        {activePanel === "debug" && (
                            <div className="p-4">
                                <h2 className="font-semibold mb-4">Debug</h2>
                                <div className="space-y-3">
                                    <label className="flex items-center gap-3 cursor-pointer">
                                        <Checkbox
                                            checked={debugOptions.showDebugSegments}
                                            onCheckedChange={(checked) => updateOption("debugOptions", {
                                                ...debugOptions,
                                                showDebugSegments: checked === true,
                                            })}
                                        />
                                        <span className="text-sm">Show vehicle route segments</span>
                                    </label>

                                    <label className="flex items-center gap-3 cursor-pointer pl-6">
                                        <Checkbox
                                            checked={debugOptions.showDebugOnlyTracked}
                                            onCheckedChange={(checked) => updateOption("debugOptions", {
                                                ...debugOptions,
                                                showDebugOnlyTracked: checked === true,
                                            })}
                                            disabled={!debugOptions.showDebugSegments}
                                        />
                                        <span className={`text-sm ${debugOptions.showDebugSegments ? "" : "text-muted-foreground"}`}>
                                            Only tracked vehicle
                                        </span>
                                    </label>
                                </div>
                            </div>
                        )}

                        {activePanel === "issues" && (
                            <OsmIssuesPanel />
                        )}

                        {activePanel === "time" && (
                            <TimeControlPanel timeSimulation={timeSimulation} />
                        )}
                    </div>
                )}
            </div>

            {/* Map */}
            <div className="flex-1 h-full">
                <Map
                    areas={areas}
                    stations={stations}
                    routes={routes}
                    vehicles={vehicles}
                    showAreaOutlines={showAreaOutlines}
                    showStations={showStations}
                    showStopPositions={showStopPositions}
                    showPlatforms={showPlatforms}
                    showRoutes={showRoutes}
                    showVehicles={showVehicles}
                    debugOptions={debugOptions}
                    simulatedTime={timeSimulation.currentTime}
                />
            </div>
        </div>
    );
}
