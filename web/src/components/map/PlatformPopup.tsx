import React from "react";
import { Departure, StopEventsResponse } from "./types";

interface PlatformPopupProps {
    platformName: string;
    stationName: string;
    platformId: string;
    osmId?: string;
    onClose: () => void;
    onStationClick: () => void;
}

interface PlatformPopupState {
    departures: Departure[];
    loading: boolean;
}

export class PlatformPopup extends React.Component<PlatformPopupProps, PlatformPopupState> {
    private intervalId?: number;

    constructor(props: PlatformPopupProps) {
        super(props);
        this.state = {
            departures: [],
            loading: true
        };
    }

    componentDidMount() {
        this.fetchDepartures();
        // Refresh every 5 seconds
        this.intervalId = window.setInterval(() => {
            this.fetchDepartures();
        }, 5000);
    }

    componentWillUnmount() {
        if (this.intervalId) {
            clearInterval(this.intervalId);
        }
    }

    fetchDepartures = async () => {
        try {
            const response = await fetch(
                `http://localhost:3000/api/stations/stop_events/${encodeURIComponent(this.props.platformId)}`
            );

            if (!response.ok) {
                console.error(`Failed to fetch departures: ${response.status}`);
                this.setState({ departures: [], loading: false });
                return;
            }

            const data: StopEventsResponse = await response.json();

            // Extract departures from the response
            const platformDepartures = data.stopEvents || [];
            this.setState({
                departures: platformDepartures.slice(0, 5), // Show max 5 departures
                loading: false
            });
        } catch (error) {
            console.error("Failed to fetch departures:", error);
            this.setState({ departures: [], loading: false });
        }
    };

    // Calculate minutes until departure
    getMinutesUntil = (departureTime?: string): number | null => {
        if (!departureTime) return null;
        const now = new Date();
        const departure = new Date(departureTime);
        const diff = Math.floor((departure.getTime() - now.getTime()) / 60000);
        return diff;
    };

    render() {
        const { platformName, stationName, platformId, osmId, onClose, onStationClick } = this.props;
        const { departures, loading } = this.state;

        // Remove "Bstg." prefix and uppercase the platform name
        const formattedPlatformName = platformName.replace(/^Bstg\.\s*/i, "").toUpperCase();

        return (
            <div className="bg-white rounded-lg shadow-lg border border-gray-200 min-w-[300px]">
                <div className="flex items-start justify-between p-4 pb-3 border-b border-gray-100">
                    <div className="flex flex-col gap-2">
                        <button
                            onClick={onStationClick}
                            className="text-xs text-gray-600 hover:text-blue-600 hover:underline cursor-pointer text-left"
                        >
                            {stationName}
                        </button>
                        <div className="w-16 h-16 rounded-full bg-gray-800 flex items-center justify-center">
                            <span className="text-white font-bold text-lg">{formattedPlatformName}</span>
                        </div>
                    </div>
                    <button
                        onClick={onClose}
                        className="ml-4 text-gray-400 hover:text-gray-600 transition-colors"
                        aria-label="Close"
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
                            <line x1="5" y1="5" x2="15" y2="15" />
                            <line x1="15" y1="5" x2="5" y2="15" />
                        </svg>
                    </button>
                </div>
                <div className="p-4 pt-3">
                    <h4 className="text-sm font-semibold mb-2">Departures</h4>
                    {loading ? (
                        <p className="text-sm text-gray-500">Loading...</p>
                    ) : departures.length === 0 ? (
                        <p className="text-sm text-gray-500">No departures</p>
                    ) : (
                        <div className="space-y-2">
                            {departures.map((dep, idx) => {
                                const minutes = this.getMinutesUntil(
                                    dep.departureTimeEstimated || dep.departureTimePlanned
                                );
                                const isDelayed = dep.departureDelay && dep.departureDelay > 0;

                                return (
                                    <div
                                        key={idx}
                                        className="flex items-center justify-between text-sm border-b border-gray-100 pb-2 last:border-0"
                                    >
                                        <div className="flex items-center gap-2">
                                            <span className="font-bold text-gray-800">
                                                {dep.transportation.number}
                                            </span>
                                            <span className="text-gray-600">
                                                {dep.transportation.destination.name}
                                            </span>
                                        </div>
                                        <div className="flex items-center gap-1">
                                            {minutes !== null && (
                                                <span
                                                    className={`font-semibold ${
                                                        isDelayed ? "text-red-600" : "text-gray-800"
                                                    }`}
                                                >
                                                    {minutes <= 0 ? "Now" : `${minutes} min`}
                                                </span>
                                            )}
                                            {isDelayed && (
                                                <span className="text-xs text-red-600">
                                                    +{dep.departureDelay}
                                                </span>
                                            )}
                                        </div>
                                    </div>
                                );
                            })}
                        </div>
                    )}
                    <div className="mt-3 pt-3 border-t border-gray-100">
                        <p className="text-xs text-gray-500">Platform ID: {platformId}</p>
                        {osmId && <p className="text-xs text-gray-500">OSM ID: {osmId}</p>}
                    </div>
                </div>
            </div>
        );
    }
}
