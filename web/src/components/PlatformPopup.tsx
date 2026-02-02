import { Terminal } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { Api, EventType, type Departure, type StationPlatform, type StationStopPosition } from "../api";
import { getConfig } from "../config";
import { formatTime, getPlatformDisplayName } from "./map/mapUtils";

let api: Api<unknown> | null = null;
function getApi() {
    if (!api) {
        api = new Api({ baseUrl: getConfig().apiUrl });
    }
    return api;
}

interface PlatformPopupProps {
    platform: StationPlatform | StationStopPosition;
    stationName?: string;
    routeColors: globalThis.Map<string, string>;
    /** When set, requests schedule-based departures for this simulated time */
    referenceTime?: Date;
}

interface TripEvent {
    tripId: string;
    lineNumber: string;
    destination: string;
    arrivalTime: string | null;
    departureTime: string | null;
    delayMinutes: number | null;
}

export function PlatformPopup({ platform, stationName, routeColors, referenceTime }: PlatformPopupProps) {
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

        getApi().api
            .getDeparturesByStop({
                stop_ifopt: platform.ref_ifopt,
                reference_time: referenceTime ? referenceTime.toISOString() : undefined,
            })
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
    }, [platform.ref_ifopt, referenceTime]);

    return (
        <div className="p-4 pr-8 bg-popover text-popover-foreground rounded-lg">
            <div className="font-semibold">Platform {displayName}</div>
            {stationName && <div className="text-sm text-muted-foreground">{stationName}</div>}

            {/* Events table */}
            <div className="mt-3 border-t border-border pt-2">
                {loading ? (
                    <div className="text-xs text-muted-foreground">Loading...</div>
                ) : tripEvents.length === 0 ? (
                    <div className="text-xs text-muted-foreground">No upcoming events</div>
                ) : (
                    <table className="text-sm">
                        <thead>
                            <tr className="text-xs text-muted-foreground">
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
                                        <td className="pr-3">{trip.destination}</td>
                                        <td className="text-muted-foreground tabular-nums pr-2">
                                            {trip.arrivalTime ? formatTime(trip.arrivalTime) : "—"}
                                        </td>
                                        <td className="text-muted-foreground tabular-nums pr-2">
                                            {trip.departureTime ? formatTime(trip.departureTime) : "—"}
                                        </td>
                                        <td>
                                            {delayMinutes > 0 && (
                                                <span className="text-destructive text-xs font-medium">+{delayMinutes}</span>
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
                className="mt-2 p-1.5 text-muted-foreground hover:text-foreground hover:bg-secondary rounded"
                title="Log to console"
            >
                <Terminal className="w-4 h-4" />
            </button>
        </div>
    );
}
