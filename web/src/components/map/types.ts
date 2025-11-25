export interface Platform {
    id: string;
    name: string;
    coord?: [number, number];
    osm_id?: string;
}

export interface Departure {
    transportation: {
        number: string;
        destination: {
            name: string;
        };
    };
    departureTimePlanned?: string;
    departureTimeEstimated?: string;
    departureDelay?: number;
}

export interface StopEventsResponse {
    version: string;
    locations: any[];
    stopEvents: Departure[];
}

export interface VehiclePosition {
    vehicle_id: string;
    line_number: string;
    line_name: string;
    destination: string;
    coordinates: [number, number];
    progress: number;
    from_station_id: string;
    to_station_id: string;
    departure_time: string;
    arrival_time: string;
    delay?: number;
    calculated_at: string;
    geometry_segment?: [number, number][];
}

export interface VehiclePositionsResponse {
    vehicles: { [vehicleId: string]: VehiclePosition };
    timestamp: string;
}

export interface Station {
    station_id: string;
    station_name: string;
    coord?: number[];
    platforms: Platform[];
}
