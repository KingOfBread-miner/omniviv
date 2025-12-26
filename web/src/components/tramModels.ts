/**
 * Tram model definitions for 3D visualization
 *
 * Each model defines the wagon configuration for a specific tram type.
 * This allows for accurate representation of different tram models
 * with their unique articulation patterns.
 */

export interface WagonSegment {
    /** Length of this wagon segment in meters */
    length: number;
    /** Type of segment for styling purposes */
    type: 'cab' | 'passenger' | 'articulation';
    /** Height of this segment in meters (for 3D extrusion) */
    height: number;
    /** Whether this segment has bogies (wheel assemblies) that track the rails */
    hasBogies: boolean;
}

export interface TramModel {
    /** Unique identifier for this model */
    id: string;
    /** Display name */
    name: string;
    /** Manufacturer */
    manufacturer: string;
    /** Total width of the tram in meters */
    width: number;
    /** Total length in meters (sum of all segments) */
    totalLength: number;
    /** Wagon segments from front to back */
    segments: WagonSegment[];
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
export const siemensCombino: TramModel = {
    id: 'siemens-combino-augsburg',
    name: 'Combino (Augsburg)',
    manufacturer: 'Siemens',
    width: 2.6,
    totalLength: 42,
    articulationGap: 0.3, // Small gap at articulation joints
    segments: [
        // Front cab section (L) - has bogies
        { length: 7.2, type: 'cab', height: 3.4, hasBogies: true },
        // Passenger section (L) - suspended, no bogies
        { length: 6.8, type: 'passenger', height: 3.4, hasBogies: false },
        // Articulation module (S) - has bogies underneath
        { length: 2.4, type: 'articulation', height: 3.2, hasBogies: true },
        // Middle passenger section (L) - suspended, no bogies
        { length: 6.8, type: 'passenger', height: 3.4, hasBogies: false },
        // Articulation module (S) - has bogies underneath
        { length: 2.4, type: 'articulation', height: 3.2, hasBogies: true },
        // Passenger section (L) - suspended, no bogies
        { length: 6.8, type: 'passenger', height: 3.4, hasBogies: false },
        // Rear cab section (L) - has bogies
        { length: 7.2, type: 'cab', height: 3.4, hasBogies: true },
    ],
};

/**
 * Generic tram model for fallback/testing
 */
export const genericTram: TramModel = {
    id: 'generic',
    name: 'Generic Tram',
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
 * Registry of all available tram models
 */
export const tramModels: Record<string, TramModel> = {
    'siemens-combino-augsburg': siemensCombino,
    'generic': genericTram,
};

/**
 * Get a tram model by ID, with fallback to generic
 */
export function getTramModel(modelId: string): TramModel {
    return tramModels[modelId] ?? genericTram;
}

/**
 * Calculate the distances from the front of the tram to each segment's
 * front and rear endpoints. Used for track-following visualization.
 *
 * @param model The tram model
 * @returns Array of { frontDistance, rearDistance, segment } for each segment
 */
export function calculateSegmentDistances(model: TramModel): Array<{
    frontDistance: number;
    rearDistance: number;
    segment: WagonSegment;
    index: number;
}> {
    const result: Array<{
        frontDistance: number;
        rearDistance: number;
        segment: WagonSegment;
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
 * Get all unique distances along the tram that need track positions.
 * This flattens the segment endpoints into a single array of distances.
 *
 * @param model The tram model
 * @returns Array of distances from front of tram
 */
export function getAllTrackDistances(model: TramModel): number[] {
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
 * Get the default tram model for Augsburg
 */
export function getAugsburgTramModel(): TramModel {
    return siemensCombino;
}
