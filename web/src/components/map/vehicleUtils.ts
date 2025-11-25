import { VehiclePosition } from "./types";

// Calculate distance along geometry
export function calculateGeometryDistances(geometry: [number, number][]): number[] {
    const distances: number[] = [0];
    let totalDistance = 0;

    for (let i = 1; i < geometry.length; i++) {
        const [lon1, lat1] = geometry[i - 1];
        const [lon2, lat2] = geometry[i];

        // Simple Euclidean distance (good enough for small segments)
        const dx = lon2 - lon1;
        const dy = lat2 - lat1;
        const distance = Math.sqrt(dx * dx + dy * dy);

        totalDistance += distance;
        distances.push(totalDistance);
    }

    return distances;
}

// Interpolate along geometry
export function interpolateAlongGeometry(
    geometry: [number, number][],
    progress: number
): [number, number] {
    if (geometry.length === 0) return [0, 0];
    if (geometry.length === 1) return geometry[0];

    const distances = calculateGeometryDistances(geometry);
    const totalDistance = distances[distances.length - 1];

    if (totalDistance === 0) return geometry[0];

    const targetDistance = progress * totalDistance;

    // Find the segment containing the target distance
    for (let i = 0; i < distances.length - 1; i++) {
        if (targetDistance >= distances[i] && targetDistance <= distances[i + 1]) {
            const segmentProgress =
                (targetDistance - distances[i]) / (distances[i + 1] - distances[i]);
            const [lon1, lat1] = geometry[i];
            const [lon2, lat2] = geometry[i + 1];

            return [lon1 + (lon2 - lon1) * segmentProgress, lat1 + (lat2 - lat1) * segmentProgress];
        }
    }

    return geometry[geometry.length - 1];
}

// Calculate current position based on time
export function calculateVehiclePosition(vehicle: VehiclePosition): [number, number] {
    const departureTime = new Date(vehicle.departure_time).getTime();
    const arrivalTime = new Date(vehicle.arrival_time).getTime();
    const currentTime = Date.now();

    // Calculate time-based progress
    const totalDuration = arrivalTime - departureTime;
    const elapsed = currentTime - departureTime;
    let progress = elapsed / totalDuration;

    // Clamp progress between 0 and 1
    progress = Math.max(0, Math.min(1, progress));

    // If we have a geometry segment, interpolate along it
    if (vehicle.geometry_segment && vehicle.geometry_segment.length > 0) {
        return interpolateAlongGeometry(vehicle.geometry_segment, progress);
    }

    // Fallback to server-calculated coordinates
    return vehicle.coordinates;
}
