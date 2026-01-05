import { useEffect, useState, useRef } from "react";
import { Badge } from "./ui/badge";
import { Activity, AlertTriangle, Clock, Zap } from "lucide-react";

interface EfaStats {
    requests_per_minute: number;
    avg_latency_ms: number;
    errors_per_minute: number;
}

interface DiagnosticsStats {
    efa: EfaStats;
}

const API_URL = import.meta.env.VITE_API_URL ?? "http://localhost:3000";

function StatCard({
    icon: Icon,
    label,
    value,
    unit,
    status,
}: {
    icon: React.ComponentType<{ className?: string }>;
    label: string;
    value: string | number;
    unit?: string;
    status?: "ok" | "warning" | "error";
}) {
    const statusColors = {
        ok: "text-green-500",
        warning: "text-yellow-500",
        error: "text-red-500",
    };

    return (
        <div className="border rounded-lg p-4 flex items-center gap-4">
            <div className={`p-2 rounded-full bg-muted ${status ? statusColors[status] : "text-muted-foreground"}`}>
                <Icon className="h-5 w-5" />
            </div>
            <div className="flex-1">
                <p className="text-sm text-muted-foreground">{label}</p>
                <p className="text-2xl font-semibold">
                    {value}
                    {unit && <span className="text-sm font-normal text-muted-foreground ml-1">{unit}</span>}
                </p>
            </div>
        </div>
    );
}

function EfaStatsSection() {
    const [stats, setStats] = useState<EfaStats | null>(null);
    const [connected, setConnected] = useState(false);
    const wsRef = useRef<WebSocket | null>(null);
    const reconnectTimeoutRef = useRef<number | null>(null);

    useEffect(() => {
        const connect = () => {
            const wsUrl = API_URL.replace(/^http/, "ws") + "/api/ws/backend-diagnostics";
            const ws = new WebSocket(wsUrl);

            ws.onopen = () => {
                console.log("Backend diagnostics WebSocket connected");
                setConnected(true);
            };

            ws.onmessage = (event) => {
                try {
                    const data = JSON.parse(event.data);
                    if (data.type === "stats") {
                        setStats(data.efa);
                    }
                } catch (e) {
                    console.error("Failed to parse diagnostics message:", e);
                }
            };

            ws.onclose = () => {
                console.log("Backend diagnostics WebSocket closed, reconnecting...");
                setConnected(false);
                reconnectTimeoutRef.current = window.setTimeout(connect, 2000);
            };

            ws.onerror = (error) => {
                console.error("Backend diagnostics WebSocket error:", error);
                setConnected(false);
            };

            wsRef.current = ws;
        };

        connect();

        return () => {
            if (wsRef.current) {
                wsRef.current.close();
            }
            if (reconnectTimeoutRef.current) {
                clearTimeout(reconnectTimeoutRef.current);
            }
        };
    }, []);

    const getLatencyStatus = (ms: number): "ok" | "warning" | "error" => {
        if (ms < 500) return "ok";
        if (ms < 1000) return "warning";
        return "error";
    };

    const getErrorStatus = (errors: number): "ok" | "warning" | "error" => {
        if (errors === 0) return "ok";
        if (errors < 5) return "warning";
        return "error";
    };

    if (!connected) {
        return (
            <div className="text-center py-8 text-muted-foreground">
                <p>Connecting to diagnostics...</p>
            </div>
        );
    }

    if (!stats) {
        return (
            <div className="text-center py-8 text-muted-foreground">
                <p>Waiting for data...</p>
            </div>
        );
    }

    return (
        <div className="space-y-4">
            <div className="flex items-center gap-2 mb-4">
                <Badge variant="outline" className="text-xs">
                    EFA API
                </Badge>
                <span className="text-xs text-muted-foreground">Last 60 seconds</span>
            </div>

            <div className="grid gap-4">
                <StatCard
                    icon={Activity}
                    label="Requests"
                    value={stats.requests_per_minute}
                    unit="/min"
                />
                <StatCard
                    icon={Zap}
                    label="Avg Latency"
                    value={stats.avg_latency_ms.toFixed(0)}
                    unit="ms"
                    status={getLatencyStatus(stats.avg_latency_ms)}
                />
                <StatCard
                    icon={AlertTriangle}
                    label="Errors"
                    value={stats.errors_per_minute}
                    unit="/min"
                    status={getErrorStatus(stats.errors_per_minute)}
                />
            </div>
        </div>
    );
}

export function BackendDiagnosticsPanel() {
    return (
        <div className="p-4 h-full flex flex-col">
            <h2 className="font-semibold mb-4">Backend Diagnostics</h2>
            <EfaStatsSection />
        </div>
    );
}
