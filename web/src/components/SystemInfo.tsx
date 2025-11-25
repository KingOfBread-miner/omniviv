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

export default function SystemInfo() {
    const [systemInfo, setSystemInfo] = useState<SystemInfo | null>(null);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState<string | null>(null);
    const [isExpanded, setIsExpanded] = useState(true);

    useEffect(() => {
        const fetchSystemInfo = async () => {
            try {
                const response = await fetch('http://localhost:3000/api/system/info');
                if (!response.ok) {
                    throw new Error(`HTTP ${response.status}`);
                }
                const data: SystemInfo = await response.json();
                setSystemInfo(data);
                setError(null);
            } catch (err) {
                console.error('Failed to fetch system info:', err);
                setError(err instanceof Error ? err.message : 'Failed to fetch');
            } finally {
                setLoading(false);
            }
        };

        fetchSystemInfo();
        // Refresh every 5 seconds
        const interval = setInterval(fetchSystemInfo, 5000);
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

            <div className="pt-2 border-t border-gray-200">
                <p className="text-xs text-gray-500">
                    Last updated: {new Date(systemInfo.timestamp).toLocaleTimeString()}
                </p>
            </div>
        </div>
    );
}
