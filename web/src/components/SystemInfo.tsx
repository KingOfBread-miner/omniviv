import { useEffect, useState } from "react";

interface EfaApiMetrics {
    total_requests: number;
    requests_last_second: number;
    requests_last_minute: number;
    avg_rps_last_minute: number;
    current_rps: number;
}

interface SystemInfo {
    efa_api_metrics: EfaApiMetrics;
    stations_monitored: number;
    efa_stations: number;
    osm_only_stations: number;
    cached_stop_events: number;
    tracked_vehicles: number;
    cache_update_interval_seconds: number;
    server_version: string;
    timestamp: string;
}

interface VehicleInfo {
    vehicle_id: string;
    trip_code: number | null;
    line_number: string;
    line_name: string;
    destination: string;
    origin: string | null;
    is_stale: boolean;
    last_seen: string;
    first_seen: string;
}

interface VehicleListResponse {
    vehicles: Record<string, VehicleInfo>;
    total_count: number;
    active_count: number;
    stale_count: number;
    timestamp: string;
}

interface PlatformInfo {
    id: string;
    name: string;
    station_name: string;
}

interface LineWithPlatforms {
    line_number: string;
    platforms: PlatformInfo[];
}

interface LinesListResponse {
    lines: LineWithPlatforms[];
    total_lines: number;
    timestamp: string;
}

export default function SystemInfo() {
    const [systemInfo, setSystemInfo] = useState<SystemInfo | null>(null);
    const [vehicleList, setVehicleList] = useState<VehicleListResponse | null>(null);
    const [linesList, setLinesList] = useState<LinesListResponse | null>(null);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState<string | null>(null);
    const [isExpanded, setIsExpanded] = useState(true);
    const [isVehiclesExpanded, setIsVehiclesExpanded] = useState(false);
    const [isLinesExpanded, setIsLinesExpanded] = useState(false);
    const [expandedLines, setExpandedLines] = useState<Set<string>>(new Set());

    useEffect(() => {
        const fetchData = async () => {
            try {
                // Fetch system info, vehicle list, and lines list in parallel
                const [systemInfoResponse, vehiclesResponse, linesResponse] = await Promise.all([
                    fetch('http://localhost:3000/api/system/info'),
                    fetch('http://localhost:3000/api/vehicles/list'),
                    fetch('http://localhost:3000/api/lines/list')
                ]);

                if (!systemInfoResponse.ok) {
                    throw new Error(`System info HTTP ${systemInfoResponse.status}`);
                }
                if (!vehiclesResponse.ok) {
                    throw new Error(`Vehicles HTTP ${vehiclesResponse.status}`);
                }
                if (!linesResponse.ok) {
                    throw new Error(`Lines HTTP ${linesResponse.status}`);
                }

                const systemData: SystemInfo = await systemInfoResponse.json();
                const vehiclesData: VehicleListResponse = await vehiclesResponse.json();
                const linesData: LinesListResponse = await linesResponse.json();

                setSystemInfo(systemData);
                setVehicleList(vehiclesData);
                setLinesList(linesData);
                setError(null);
            } catch (err) {
                console.error('Failed to fetch data:', err);
                setError(err instanceof Error ? err.message : 'Failed to fetch');
            } finally {
                setLoading(false);
            }
        };

        fetchData();
        // Refresh every 5 seconds
        const interval = setInterval(fetchData, 5000);
        return () => clearInterval(interval);
    }, []);

    // Collapsed view
    if (!isExpanded) {
        return (
            <button
                onClick={() => setIsExpanded(true)}
                className="bg-white rounded-lg shadow-lg border border-gray-200 p-3 hover:bg-gray-50 transition-colors"
                aria-label="Expand system information"
            >
                <div className="flex items-center gap-2">
                    <svg
                        width="16"
                        height="16"
                        viewBox="0 0 16 16"
                        fill="none"
                        stroke="currentColor"
                        strokeWidth="2"
                        strokeLinecap="round"
                        className="text-gray-600"
                    >
                        <circle cx="8" cy="8" r="6" />
                        <path d="M8 6v4M6 8h4" />
                    </svg>
                    {systemInfo && (
                        <div className="flex items-center gap-2">
                            <span className="text-sm font-semibold text-blue-700">
                                {systemInfo.efa_api_metrics.current_rps.toFixed(1)} RPS
                            </span>
                            <span className="text-xs text-gray-500">•</span>
                            <span className="text-sm font-semibold text-blue-600">
                                {systemInfo.tracked_vehicles} 🚊
                            </span>
                        </div>
                    )}
                </div>
            </button>
        );
    }

    if (loading) {
        return (
            <div className="bg-white rounded-lg shadow-lg border border-gray-200 p-4">
                <div className="flex items-center justify-between mb-2">
                    <p className="text-sm text-gray-500">Loading system info...</p>
                    <button
                        onClick={() => setIsExpanded(false)}
                        className="text-gray-400 hover:text-gray-600 transition-colors"
                        aria-label="Collapse"
                    >
                        <svg
                            width="20"
                            height="20"
                            viewBox="0 0 20 20"
                            fill="none"
                            stroke="currentColor"
                            strokeWidth="2"
                            strokeLinecap="round"
                        >
                            <path d="M5 8l5 5 5-5" />
                        </svg>
                    </button>
                </div>
            </div>
        );
    }

    if (error) {
        return (
            <div className="bg-white rounded-lg shadow-lg border border-red-200 p-4">
                <div className="flex items-center justify-between mb-2">
                    <p className="text-sm text-red-600">Error: {error}</p>
                    <button
                        onClick={() => setIsExpanded(false)}
                        className="text-gray-400 hover:text-gray-600 transition-colors"
                        aria-label="Collapse"
                    >
                        <svg
                            width="20"
                            height="20"
                            viewBox="0 0 20 20"
                            fill="none"
                            stroke="currentColor"
                            strokeWidth="2"
                            strokeLinecap="round"
                        >
                            <path d="M5 8l5 5 5-5" />
                        </svg>
                    </button>
                </div>
            </div>
        );
    }

    if (!systemInfo) {
        return null;
    }

    return (
        <div className="bg-white rounded-lg shadow-lg border border-gray-200 p-4 space-y-3">
            <div className="flex items-center justify-between pb-2 border-b border-gray-200">
                <div className="flex items-center gap-2">
                    <h3 className="font-bold text-lg">System Information</h3>
                    <span className="text-xs text-gray-500">v{systemInfo.server_version}</span>
                </div>
                <button
                    onClick={() => setIsExpanded(false)}
                    className="text-gray-400 hover:text-gray-600 transition-colors"
                    aria-label="Collapse"
                >
                    <svg
                        width="20"
                        height="20"
                        viewBox="0 0 20 20"
                        fill="none"
                        stroke="currentColor"
                        strokeWidth="2"
                        strokeLinecap="round"
                    >
                        <path d="M5 8l5 5 5-5" />
                    </svg>
                </button>
            </div>

            {/* EFA API Metrics */}
            <div>
                <h4 className="font-semibold text-sm mb-2 text-gray-700">EFA API Metrics</h4>
                <div className="grid grid-cols-2 gap-2 text-sm">
                    <div className="bg-blue-50 rounded p-2">
                        <div className="text-xs text-gray-600">Current RPS</div>
                        <div className="text-lg font-bold text-blue-700">
                            {systemInfo.efa_api_metrics.current_rps.toFixed(1)}
                        </div>
                    </div>
                    <div className="bg-green-50 rounded p-2">
                        <div className="text-xs text-gray-600">Avg RPS (1m)</div>
                        <div className="text-lg font-bold text-green-700">
                            {systemInfo.efa_api_metrics.avg_rps_last_minute.toFixed(2)}
                        </div>
                    </div>
                    <div className="bg-purple-50 rounded p-2">
                        <div className="text-xs text-gray-600">Requests (1s)</div>
                        <div className="text-lg font-bold text-purple-700">
                            {systemInfo.efa_api_metrics.requests_last_second}
                        </div>
                    </div>
                    <div className="bg-orange-50 rounded p-2">
                        <div className="text-xs text-gray-600">Requests (1m)</div>
                        <div className="text-lg font-bold text-orange-700">
                            {systemInfo.efa_api_metrics.requests_last_minute}
                        </div>
                    </div>
                    <div className="bg-gray-50 rounded p-2 col-span-2">
                        <div className="text-xs text-gray-600">Total Requests</div>
                        <div className="text-lg font-bold text-gray-700">
                            {systemInfo.efa_api_metrics.total_requests.toLocaleString()}
                        </div>
                    </div>
                </div>
            </div>

            {/* Station Metrics */}
            <div>
                <h4 className="font-semibold text-sm mb-2 text-gray-700">Network Status</h4>
                <div className="space-y-1 text-sm">
                    <div className="flex justify-between">
                        <span className="text-gray-600">Stations Monitored:</span>
                        <span className="font-semibold">{systemInfo.stations_monitored}</span>
                    </div>
                    <div className="flex justify-between">
                        <span className="text-gray-600">EFA Stations:</span>
                        <span className="font-semibold text-green-600">{systemInfo.efa_stations}</span>
                    </div>
                    <div className="flex justify-between">
                        <span className="text-gray-600">OSM-only Stations:</span>
                        <span className="font-semibold text-gray-500">{systemInfo.osm_only_stations}</span>
                    </div>
                    <div className="flex justify-between">
                        <span className="text-gray-600">Cached Stop Events:</span>
                        <span className="font-semibold">{systemInfo.cached_stop_events}</span>
                    </div>
                    <div className="flex justify-between">
                        <span className="text-gray-600">Tracked Vehicles:</span>
                        <span className="font-semibold text-blue-600">{systemInfo.tracked_vehicles}</span>
                    </div>
                    <div className="flex justify-between">
                        <span className="text-gray-600">Cache Update:</span>
                        <span className="font-semibold">{systemInfo.cache_update_interval_seconds}s</span>
                    </div>
                </div>
            </div>

            {/* Vehicles List */}
            <div>
                <button
                    onClick={() => setIsVehiclesExpanded(!isVehiclesExpanded)}
                    className="w-full flex items-center justify-between hover:bg-gray-50 rounded p-2 transition-colors"
                >
                    <div className="flex items-center gap-2">
                        <h4 className="font-semibold text-sm text-gray-700">
                            All Vehicles ({vehicleList?.total_count || 0})
                        </h4>
                        {vehicleList && (
                            <div className="flex items-center gap-1 text-xs">
                                <span className="text-green-600 font-semibold">
                                    {vehicleList.active_count} active
                                </span>
                                {vehicleList.stale_count > 0 && (
                                    <>
                                        <span className="text-gray-400">•</span>
                                        <span className="text-orange-600 font-semibold">
                                            {vehicleList.stale_count} stale
                                        </span>
                                    </>
                                )}
                            </div>
                        )}
                    </div>
                    <svg
                        width="16"
                        height="16"
                        viewBox="0 0 16 16"
                        fill="none"
                        stroke="currentColor"
                        strokeWidth="2"
                        strokeLinecap="round"
                        className={`transition-transform ${isVehiclesExpanded ? 'rotate-180' : ''}`}
                    >
                        <path d="M4 6l4 4 4-4" />
                    </svg>
                </button>

                {isVehiclesExpanded && vehicleList && (
                    <div className="mt-2 space-y-2 max-h-96 overflow-y-auto">
                        {Object.entries(vehicleList.vehicles)
                            .sort(([, a], [, b]) => {
                                // Sort: active first, then by line number, then by destination
                                if (a.is_stale !== b.is_stale) {
                                    return a.is_stale ? 1 : -1;
                                }
                                const lineCompare = parseInt(a.line_number) - parseInt(b.line_number);
                                if (lineCompare !== 0) return lineCompare;
                                return a.destination.localeCompare(b.destination);
                            })
                            .map(([vehicleId, vehicle]) => (
                                <div
                                    key={vehicleId}
                                    className={`rounded p-2 text-xs border ${
                                        vehicle.is_stale
                                            ? 'bg-orange-50 border-orange-300'
                                            : 'bg-gray-50 border-gray-200'
                                    }`}
                                >
                                    <div className="flex items-start justify-between gap-2">
                                        <div className="flex-1 min-w-0">
                                            <div className="flex items-center gap-2 mb-1">
                                                <span className={`inline-flex items-center justify-center w-6 h-6 rounded-full text-white font-bold text-xs ${
                                                    vehicle.is_stale ? 'bg-orange-500 opacity-60' : 'bg-blue-600'
                                                }`}>
                                                    {vehicle.line_number}
                                                </span>
                                                <span className={`font-semibold truncate ${
                                                    vehicle.is_stale ? 'text-gray-600' : 'text-gray-900'
                                                }`}>
                                                    → {vehicle.destination}
                                                </span>
                                                {vehicle.is_stale && (
                                                    <span className="text-xs font-semibold text-orange-600 bg-orange-100 px-1.5 py-0.5 rounded">
                                                        STALE
                                                    </span>
                                                )}
                                            </div>
                                            {vehicle.origin && (
                                                <div className={`truncate ml-8 ${
                                                    vehicle.is_stale ? 'text-gray-500' : 'text-gray-600'
                                                }`}>
                                                    from {vehicle.origin}
                                                </div>
                                            )}
                                            <div className="text-gray-500 truncate ml-8 mt-1 text-[10px]">
                                                ID: {vehicleId}
                                            </div>
                                            {vehicle.is_stale && (
                                                <div className="text-orange-600 truncate ml-8 mt-1 text-[10px]">
                                                    Last seen: {new Date(vehicle.last_seen).toLocaleTimeString()}
                                                </div>
                                            )}
                                        </div>
                                    </div>
                                </div>
                            ))}
                    </div>
                )}
            </div>

            {/* Tram Lines List */}
            <div>
                <button
                    onClick={() => setIsLinesExpanded(!isLinesExpanded)}
                    className="w-full flex items-center justify-between hover:bg-gray-50 rounded p-2 transition-colors"
                >
                    <div className="flex items-center gap-2">
                        <h4 className="font-semibold text-sm text-gray-700">
                            Tram Lines ({linesList?.total_lines || 0})
                        </h4>
                    </div>
                    <svg
                        width="16"
                        height="16"
                        viewBox="0 0 16 16"
                        fill="none"
                        stroke="currentColor"
                        strokeWidth="2"
                        strokeLinecap="round"
                        className={`transition-transform ${isLinesExpanded ? 'rotate-180' : ''}`}
                    >
                        <path d="M4 6l4 4 4-4" />
                    </svg>
                </button>

                {isLinesExpanded && linesList && (
                    <div className="mt-2 space-y-2 max-h-96 overflow-y-auto">
                        {linesList.lines.filter(line => line.platforms && line.platforms.length > 0).map((line) => (
                            <div key={line.line_number} className="rounded border border-gray-200">
                                <button
                                    onClick={() => {
                                        const newExpanded = new Set(expandedLines);
                                        if (newExpanded.has(line.line_number)) {
                                            newExpanded.delete(line.line_number);
                                        } else {
                                            newExpanded.add(line.line_number);
                                        }
                                        setExpandedLines(newExpanded);
                                    }}
                                    className="w-full flex items-center justify-between p-2 hover:bg-gray-50 transition-colors"
                                >
                                    <div className="flex items-center gap-2">
                                        <span className="inline-flex items-center justify-center w-8 h-8 rounded-full bg-blue-600 text-white font-bold text-sm">
                                            {line.line_number}
                                        </span>
                                        <span className="text-sm text-gray-600">
                                            {line.platforms?.length || 0} platforms
                                        </span>
                                    </div>
                                    <svg
                                        width="14"
                                        height="14"
                                        viewBox="0 0 16 16"
                                        fill="none"
                                        stroke="currentColor"
                                        strokeWidth="2"
                                        strokeLinecap="round"
                                        className={`transition-transform ${expandedLines.has(line.line_number) ? 'rotate-180' : ''}`}
                                    >
                                        <path d="M4 6l4 4 4-4" />
                                    </svg>
                                </button>

                                {expandedLines.has(line.line_number) && line.platforms && (
                                    <div className="px-2 pb-2 space-y-1">
                                        {line.platforms.map((platform, idx) => (
                                            <div
                                                key={platform.id}
                                                className="text-xs bg-gray-50 rounded px-2 py-1.5"
                                            >
                                                <div className="flex items-start gap-2">
                                                    <span className="text-gray-400 font-semibold mt-0.5">{idx + 1}.</span>
                                                    <div className="flex-1">
                                                        <div className="text-gray-900 font-semibold">{platform.station_name}</div>
                                                        <div className="text-gray-600 text-[11px] mt-0.5">{platform.name}</div>
                                                        <div className="text-gray-400 font-mono text-[10px] mt-0.5">{platform.id}</div>
                                                    </div>
                                                </div>
                                            </div>
                                        ))}
                                    </div>
                                )}
                            </div>
                        ))}
                    </div>
                )}
            </div>

            <div className="pt-2 border-t border-gray-200">
                <p className="text-xs text-gray-500">
                    Last updated: {new Date(systemInfo.timestamp).toLocaleTimeString()}
                </p>
            </div>
        </div>
    );
}
