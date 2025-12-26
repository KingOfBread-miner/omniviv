import { useCallback, useEffect, useState } from "react";
import { Api, Area, Route, RouteGeometry, Station, Vehicle } from "./api";
import { IssuesPanel } from "./components/IssuesPanel";
import Map from "./components/map/Map";

const API_URL = import.meta.env.VITE_API_URL ?? "http://localhost:3000";
const api = new Api({ baseUrl: API_URL });

// How often to refresh vehicle positions (in milliseconds)
const VEHICLE_REFRESH_INTERVAL = 5000;

export interface RouteWithGeometry extends Route {
    geometry: RouteGeometry | null;
}

export interface RouteVehicles {
    routeId: number;
    lineNumber: string | null;
    vehicles: Vehicle[];
}

export default function App() {
    const [areas, setAreas] = useState<Area[]>([]);
    const [stations, setStations] = useState<Station[]>([]);
    const [routes, setRoutes] = useState<RouteWithGeometry[]>([]);
    const [vehicles, setVehicles] = useState<RouteVehicles[]>([]);
    const [menuOpen, setMenuOpen] = useState(false);
    const [showAreaOutlines, setShowAreaOutlines] = useState(false);
    const [showStations, setShowStations] = useState(true);
    const [showRoutes, setShowRoutes] = useState(true);
    const [showVehicles, setShowVehicles] = useState(true);

    // Fetch vehicles for all routes
    const fetchVehicles = useCallback(async (routeList: RouteWithGeometry[]) => {
        if (routeList.length === 0) return;

        try {
            const vehiclePromises = routeList.map(async (route) => {
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

                // Initial vehicle fetch
                await fetchVehicles(routesWithGeometry);
            } catch (err) {
                console.error("Failed to fetch data:", err);
            }
        };

        fetchData();
    }, [fetchVehicles]);

    // Periodic vehicle refresh
    useEffect(() => {
        if (routes.length === 0 || !showVehicles) return;

        const interval = setInterval(() => {
            fetchVehicles(routes);
        }, VEHICLE_REFRESH_INTERVAL);

        return () => clearInterval(interval);
    }, [routes, showVehicles, fetchVehicles]);

    return (
        <div className="h-screen w-screen relative">
            {/* Map */}
            <Map
                areas={areas}
                stations={stations}
                routes={routes}
                vehicles={vehicles}
                showAreaOutlines={showAreaOutlines}
                showStations={showStations}
                showRoutes={showRoutes}
                showVehicles={showVehicles}
            />

            {/* Issues Panel */}
            <IssuesPanel />

            {/* Burger Menu Button */}
            <button
                onClick={() => setMenuOpen(!menuOpen)}
                className="absolute top-4 left-4 z-20 bg-white rounded-lg shadow-lg p-3 hover:bg-gray-50 transition-colors"
                aria-label="Toggle menu"
            >
                <svg
                    width="24"
                    height="24"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    strokeWidth="2"
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    className="text-gray-700"
                >
                    {menuOpen ? (
                        <>
                            <path d="M18 6L6 18" />
                            <path d="M6 6l12 12" />
                        </>
                    ) : (
                        <>
                            <path d="M4 6h16" />
                            <path d="M4 12h16" />
                            <path d="M4 18h16" />
                        </>
                    )}
                </svg>
            </button>

            {/* Menu Panel */}
            {menuOpen && (
                <div className="absolute top-16 left-4 z-20 bg-white rounded-lg shadow-lg p-4 min-w-64">
                    <h2 className="font-semibold text-gray-900 mb-4">Map Options</h2>

                    <label className="flex items-center gap-3 cursor-pointer hover:bg-gray-50 p-2 rounded -mx-2">
                        <input
                            type="checkbox"
                            checked={showAreaOutlines}
                            onChange={(e) => setShowAreaOutlines(e.target.checked)}
                            className="w-5 h-5 rounded border-gray-300 text-blue-600 focus:ring-blue-500"
                        />
                        <span className="text-gray-700">Show area outlines</span>
                    </label>

                    <label className="flex items-center gap-3 cursor-pointer hover:bg-gray-50 p-2 rounded -mx-2">
                        <input
                            type="checkbox"
                            checked={showStations}
                            onChange={(e) => setShowStations(e.target.checked)}
                            className="w-5 h-5 rounded border-gray-300 text-blue-600 focus:ring-blue-500"
                        />
                        <span className="text-gray-700">Show stations ({stations.length})</span>
                    </label>

                    <label className="flex items-center gap-3 cursor-pointer hover:bg-gray-50 p-2 rounded -mx-2">
                        <input
                            type="checkbox"
                            checked={showRoutes}
                            onChange={(e) => setShowRoutes(e.target.checked)}
                            className="w-5 h-5 rounded border-gray-300 text-blue-600 focus:ring-blue-500"
                        />
                        <span className="text-gray-700">Show routes ({routes.length})</span>
                    </label>

                    <label className="flex items-center gap-3 cursor-pointer hover:bg-gray-50 p-2 rounded -mx-2">
                        <input
                            type="checkbox"
                            checked={showVehicles}
                            onChange={(e) => setShowVehicles(e.target.checked)}
                            className="w-5 h-5 rounded border-gray-300 text-blue-600 focus:ring-blue-500"
                        />
                        <span className="text-gray-700">
                            Show vehicles ({vehicles.reduce((acc, rv) => acc + rv.vehicles.length, 0)})
                        </span>
                    </label>

                    {areas.length > 0 && (
                        <div className="mt-4 pt-4 border-t border-gray-200">
                            <h3 className="text-sm font-medium text-gray-500 mb-2">Areas</h3>
                            <ul className="space-y-1">
                                {areas.map((area) => (
                                    <li
                                        key={area.id}
                                        className="text-sm text-gray-700 flex items-center gap-2"
                                    >
                                        <span className="w-2 h-2 rounded-full bg-blue-500" />
                                        {area.name}
                                    </li>
                                ))}
                            </ul>
                        </div>
                    )}
                </div>
            )}
        </div>
    );
}
