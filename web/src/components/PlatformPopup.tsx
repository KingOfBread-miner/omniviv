import { Terminal } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { Api, EventType, type Departure, type StationPlatform, type StationStopPosition } from "../api";
import { formatTime, getPlatformDisplayName } from "./map/mapUtils";

const API_URL = import.meta.env.VITE_API_URL ?? "http://localhost:3000";
const api = new Api({ baseUrl: API_URL });

interface PlatformPopupProps {
    platform: StationPlatform | StationStopPosition;
    stationName?: string;
    routeColors: globalThis.Map<string, string>;
}

interface TripEvent {
    tripId: string;
    lineNumber: string;
    destination: string;
    arrivalTime: string | null;
    departureTime: string | null;
    delayMinutes: number | null;
}

export function PlatformPopup({ platform, stationName, routeColors }: PlatformPopupProps) {
    const [events, setEvents] = useState<Departure[]>([]);
    const [loading, setLoading] = useState(true);
    const displayName = getPlatformDisplayName(platform);

    // Group arrivals and departures by trip
    const tripEvents = useMemo(() => {
        const tripMap = new Map<string, TripEvent>();

        for (const event of events) {
            const existing = tripMap.get(event.trip_id);
            const time = event.estimated_time || event.planned_time;

            if (existing) {
                if (event.event_type === EventType.Arrival) {
                    existing.arrivalTime = time;
                } else {
                    existing.departureTime = time;
                }
                // Use the most recent delay info
                if (event.delay_minutes !== null) {
                    existing.delayMinutes = event.delay_minutes;
                }
            } else {
                tripMap.set(event.trip_id, {
                    tripId: event.trip_id,
                    lineNumber: event.line_number,
                    destination: event.destination,
                    arrivalTime: event.event_type === EventType.Arrival ? time : null,
                    departureTime: event.event_type === EventType.Departure ? time : null,
                    delayMinutes: event.delay_minutes ?? null,
                });
            }
        }

        // Sort by earliest time (arrival or departure)
        return Array.from(tripMap.values()).sort((a, b) => {
            const timeA = a.arrivalTime || a.departureTime || "";
            const timeB = b.arrivalTime || b.departureTime || "";
            return timeA.localeCompare(timeB);
        });
    }, [events]);

    useEffect(() => {
        if (!platform.ref_ifopt) {
            setLoading(false);
            return;
        }

        api.api
            .getDeparturesByStop({ stop_ifopt: platform.ref_ifopt })
            .then((res) => {
                setEvents(res.data?.departures ?? []);
            })
            .catch((err) => {
                console.error("Failed to fetch departures:", err);
                setEvents([]);
            })
            .finally(() => {
                setLoading(false);
            });
    }, [platform.ref_ifopt]);

    return (
        <div className="p-4 pr-8">
            <div className="font-semibold text-gray-900">Platform {displayName}</div>
            {stationName && <div className="text-sm text-gray-600">{stationName}</div>}

            {/* Events table */}
            <div className="mt-3 border-t pt-2">
                {loading ? (
                    <div className="text-xs text-gray-500">Loading...</div>
                ) : tripEvents.length === 0 ? (
                    <div className="text-xs text-gray-500">No upcoming events</div>
                ) : (
                    <table className="text-sm">
                        <thead>
                            <tr className="text-xs text-gray-500">
                                <th className="text-left font-medium pr-2">Line</th>
                                <th className="text-left font-medium pr-3">Destination</th>
                                <th className="text-left font-medium pr-2">Arrival</th>
                                <th className="text-left font-medium pr-2">Departure</th>
                                <th className="text-left font-medium"></th>
                            </tr>
                        </thead>
                        <tbody>
                            {tripEvents.slice(0, 8).map((trip) => {
                                const color = routeColors.get(trip.lineNumber) || "#6b7280";
                                const delayMinutes = trip.delayMinutes ?? 0;
                                return (
                                    <tr key={trip.tripId} className="whitespace-nowrap">
                                        <td className="font-mono font-semibold pr-2" style={{ color }}>
                                            {trip.lineNumber}
                                        </td>
                                        <td className="text-gray-700 pr-3">{trip.destination}</td>
                                        <td className="text-gray-500 tabular-nums pr-2">
                                            {trip.arrivalTime ? formatTime(trip.arrivalTime) : "—"}
                                        </td>
                                        <td className="text-gray-500 tabular-nums pr-2">
                                            {trip.departureTime ? formatTime(trip.departureTime) : "—"}
                                        </td>
                                        <td>
                                            {delayMinutes > 0 && (
                                                <span className="text-red-500 text-xs font-medium">+{delayMinutes}</span>
                                            )}
                                        </td>
                                    </tr>
                                );
                            })}
                        </tbody>
                    </table>
                )}
            </div>

            <button
                onClick={() => console.log("Platform:", platform, "Events:", tripEvents)}
                className="mt-2 p-1.5 text-gray-400 hover:text-gray-600 hover:bg-gray-100 rounded"
                title="Log to console"
            >
                <Terminal className="w-4 h-4" />
            </button>
        </div>
    );
}
