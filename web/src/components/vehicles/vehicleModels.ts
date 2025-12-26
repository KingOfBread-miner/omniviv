/**
 * Vehicle model definitions for 3D visualization
 *
 * Each model defines the wagon configuration for a specific vehicle type.
 * This allows for accurate representation of different vehicle models
 * with their unique articulation patterns.
 */

export interface VehicleSegment {
    /** Length of this segment in meters */
    length: number;
    /** Type of segment for styling purposes */
    type: 'cab' | 'passenger' | 'articulation';
    /** Height of this segment in meters (for 3D extrusion) */
    height: number;
    /** Whether this segment has bogies (wheel assemblies) that track the rails */
    hasBogies: boolean;
}

export interface VehicleModel {
    /** Unique identifier for this model */
    id: string;
    /** Display name */
    name: string;
    /** Manufacturer */
    manufacturer: string;
    /** Total width of the vehicle in meters */
    width: number;
    /** Total length in meters (sum of all segments) */
    totalLength: number;
    /** Segments from front to back */
    segments: VehicleSegment[];
    /** Gap between segments in meters (for articulation joints) */
    articulationGap: number;
}

/**
 * Siemens Combino - Augsburg variant
 *
 * 7-section low-floor tram with pattern: L,L,S,L,S,L,L
 * Total length: 42m, Width: 2.6m
 *
 * The short sections (S) are articulation modules with bogies underneath.
 * The cab sections also have bogies. Passenger sections are suspended
 * between the bogied sections (floating low-floor design).
 *
 * Bogie positions: Front cab, Articulation 1, Articulation 2, Rear cab
 */
export const siemensCombino: VehicleModel = {
    id: 'siemens-combino-augsburg',
    name: 'Combino (Augsburg)',
    manufacturer: 'Siemens',
    width: 2.6,
    totalLength: 42,
    articulationGap: 0.3,
    segments: [
        { length: 7.2, type: 'cab', height: 3.4, hasBogies: true },
        { length: 6.8, type: 'passenger', height: 3.4, hasBogies: false },
        { length: 2.4, type: 'articulation', height: 3.2, hasBogies: true },
        { length: 6.8, type: 'passenger', height: 3.4, hasBogies: false },
        { length: 2.4, type: 'articulation', height: 3.2, hasBogies: true },
        { length: 6.8, type: 'passenger', height: 3.4, hasBogies: false },
        { length: 7.2, type: 'cab', height: 3.4, hasBogies: true },
    ],
};

/**
 * Generic vehicle model for fallback/testing
 */
export const genericVehicle: VehicleModel = {
    id: 'generic',
    name: 'Generic Vehicle',
    manufacturer: 'Generic',
    width: 2.4,
    totalLength: 30,
    articulationGap: 0.2,
    segments: [
        { length: 15, type: 'cab', height: 3.5, hasBogies: true },
        { length: 15, type: 'cab', height: 3.5, hasBogies: true },
    ],
};

/**
 * Registry of all available vehicle models
 */
export const vehicleModels: Record<string, VehicleModel> = {
    'siemens-combino-augsburg': siemensCombino,
    'generic': genericVehicle,
};

/**
 * Get a vehicle model by ID, with fallback to generic
 */
export function getVehicleModel(modelId: string): VehicleModel {
    return vehicleModels[modelId] ?? genericVehicle;
}

/**
 * Calculate the distances from the front of the vehicle to each segment's
 * front and rear endpoints. Used for track-following visualization.
 */
export function calculateSegmentDistances(model: VehicleModel): Array<{
    frontDistance: number;
    rearDistance: number;
    segment: VehicleSegment;
    index: number;
}> {
    const result: Array<{
        frontDistance: number;
        rearDistance: number;
        segment: VehicleSegment;
        index: number;
    }> = [];
    let currentDistance = 0;

    for (let i = 0; i < model.segments.length; i++) {
        const segment = model.segments[i];
        result.push({
            frontDistance: currentDistance,
            rearDistance: currentDistance + segment.length,
            segment,
            index: i,
        });
        currentDistance += segment.length + model.articulationGap;
    }

    return result;
}

/**
 * Get all unique distances along the vehicle that need track positions.
 */
export function getAllTrackDistances(model: VehicleModel): number[] {
    const distances = new Set<number>();
    let currentDistance = 0;

    for (const segment of model.segments) {
        distances.add(currentDistance);
        distances.add(currentDistance + segment.length);
        currentDistance += segment.length + model.articulationGap;
    }

    return Array.from(distances).sort((a, b) => a - b);
}

/**
 * Get the default vehicle model for Augsburg
 */
export function getAugsburgVehicleModel(): VehicleModel {
    return siemensCombino;
}
