/**
 * Handles vehicle position animation, marker rendering, and 3D model visualization
 */

import type { RouteVehicles } from "../../App";
import type { MapLayerManager } from "../map/MapLayerManager";
import { createVehicleIcon } from "./VehicleIconFactory";
import { calculateSegmentDistances, getAugsburgVehicleModel } from "./vehicleModels";
import {
    calculateVehiclePosition,
    createSmoothedPosition,
    getDebugSegmentFeatures,
    getPositionsBehindOnRoute,
    linearizeRoute,
    updateSmoothedPosition,
    type LinearizedRoute,
    type SmoothedVehiclePosition,
    type VehiclePosition,
} from "./vehicleUtils";

const ANIMATION_INTERVAL = 50;

export interface DebugOptions {
    show3DModels: boolean;
    showDebugSegments: boolean;
    showDebugOnlyTracked: boolean;
}

export class VehicleRenderer {
    private layerManager: MapLayerManager;
    private routeColors: globalThis.Map<string, string>;
    private routeGeometries: globalThis.Map<number, number[][][]>;
    private linearizedRoutes = new globalThis.Map<number, LinearizedRoute>();
    private smoothedPositions = new globalThis.Map<string, SmoothedVehiclePosition>();
    private vehicleIcons = new Set<string>();
    private animationId: number | null = null;
    private lastAnimationTime = 0;

    private onTrackedVehicleLost?: () => void;
    private trackedTripId: string | null = null;
    private debugOptions: DebugOptions = { show3DModels: true, showDebugSegments: false, showDebugOnlyTracked: true };

    constructor(
        layerManager: MapLayerManager,
        routeColors: globalThis.Map<string, string>,
        routeGeometries: globalThis.Map<number, number[][][]>
    ) {
        this.layerManager = layerManager;
        this.routeColors = routeColors;
        this.routeGeometries = routeGeometries;
        this.buildLinearizedRoutes();
    }

    /**
     * Build linearized routes from route geometries
     */
    private buildLinearizedRoutes(): void {
        this.linearizedRoutes.clear();
        for (const [routeId, geometry] of this.routeGeometries) {
            const linearized = linearizeRoute(geometry);
            if (linearized) {
                this.linearizedRoutes.set(routeId, linearized);
            }
        }
    }

    /**
     * Update route data references
     */
    updateRouteData(
        routeColors: globalThis.Map<string, string>,
        routeGeometries: globalThis.Map<number, number[][][]>
    ): void {
        this.routeColors = routeColors;
        this.routeGeometries = routeGeometries;
        this.buildLinearizedRoutes();
    }

    /**
     * Set callback for when tracked vehicle is lost
     */
    setOnTrackedVehicleLost(callback: () => void): void {
        this.onTrackedVehicleLost = callback;
    }

    /**
     * Set the currently tracked trip ID
     */
    setTrackedTripId(tripId: string | null): void {
        this.trackedTripId = tripId;
    }

    /**
     * Set debug visualization options
     */
    setDebugOptions(options: DebugOptions): void {
        this.debugOptions = options;
    }

    /**
     * Get a specific smoothed position
     */
    getSmoothedPosition(tripId: string): SmoothedVehiclePosition | undefined {
        return this.smoothedPositions.get(tripId);
    }

    /**
     * Start the vehicle animation loop
     */
    startAnimation(vehicles: RouteVehicles[]): void {
        if (this.animationId) return;

        this.updatePositions(vehicles, ANIMATION_INTERVAL);

        const animate = (timestamp: number) => {
            const deltaMs = this.lastAnimationTime > 0 ? timestamp - this.lastAnimationTime : ANIMATION_INTERVAL;
            if (deltaMs >= ANIMATION_INTERVAL) {
                this.lastAnimationTime = timestamp;
                this.updatePositions(vehicles, deltaMs);
            }
            this.animationId = requestAnimationFrame(animate);
        };

        this.animationId = requestAnimationFrame(animate);
    }

    /**
     * Stop the vehicle animation loop
     */
    stopAnimation(): void {
        if (this.animationId) {
            cancelAnimationFrame(this.animationId);
            this.animationId = null;
        }
        this.lastAnimationTime = 0;
    }

    /**
     * Clear all vehicle data
     */
    clear(): void {
        this.stopAnimation();
        this.layerManager.clearVehicleData();
        this.layerManager.updateDebugSegments([]);
        this.smoothedPositions.clear();
    }

    /**
     * Update vehicle positions, markers, and 3D models in a single pass
     */
    updatePositions(vehicles: RouteVehicles[], deltaMs: number): void {
        const now = new Date();
        const vehiclesByTripId = new globalThis.Map<string, { vehicle: RouteVehicles["vehicles"][0]; routeId: number; stopCount: number }>();

        for (const routeVehicles of vehicles) {
            for (const vehicle of routeVehicles.vehicles) {
                const existing = vehiclesByTripId.get(vehicle.trip_id);
                if (!existing || vehicle.stops.length > existing.stopCount) {
                    vehiclesByTripId.set(vehicle.trip_id, { vehicle, routeId: routeVehicles.routeId, stopCount: vehicle.stops.length });
                }
            }
        }

        const allPositions: { position: VehiclePosition; routeId: number; routeColor: string }[] = [];
        const completingAtLocation = new Set<string>();

        for (const { vehicle, routeId } of vehiclesByTripId.values()) {
            const routeGeometry = this.routeGeometries.get(routeId);
            const routeColor = this.routeColors.get(vehicle.line_number ?? "") ?? "#3b82f6";
            const targetPosition = calculateVehiclePosition(vehicle, routeGeometry ?? [], now);

            if (targetPosition && targetPosition.status !== "completed") {
                allPositions.push({ position: targetPosition, routeId, routeColor });
                const lastStop = vehicle.stops[vehicle.stops.length - 1];
                const isOnFinalSegment = targetPosition.nextStop?.stop_ifopt === lastStop?.stop_ifopt;
                if (isOnFinalSegment && targetPosition.progress > 0.5 && lastStop?.stop_ifopt) {
                    completingAtLocation.add(`${targetPosition.lineNumber}:${lastStop.stop_ifopt}`);
                }
            }
        }

        const markerFeatures: GeoJSON.Feature[] = [];
        const modelFeatures: GeoJSON.Feature[] = [];
        const debugFeatures: GeoJSON.Feature[] = [];
        const activeTripIds = new Set<string>();

        const vehicleModel = getAugsburgVehicleModel();
        const segmentDistances = calculateSegmentDistances(vehicleModel);

        for (const { position: targetPosition, routeId, routeColor } of allPositions) {
            // Skip waiting vehicles unless another vehicle is completing at same location
            if (targetPosition.status === "waiting") {
                const vehicle = vehiclesByTripId.get(targetPosition.tripId)?.vehicle;
                const firstStop = vehicle?.stops[0];
                const locationKey = `${targetPosition.lineNumber}:${firstStop?.stop_ifopt}`;
                if (!completingAtLocation.has(locationKey)) continue;
            }

            activeTripIds.add(targetPosition.tripId);

            // Update smoothed position
            let smoothedPosition = this.smoothedPositions.get(targetPosition.tripId);
            if (smoothedPosition) {
                smoothedPosition = updateSmoothedPosition(smoothedPosition, targetPosition, deltaMs);
            } else {
                smoothedPosition = createSmoothedPosition(targetPosition);
            }
            this.smoothedPositions.set(targetPosition.tripId, smoothedPosition);

            // Create vehicle marker icon
            const lineNum = smoothedPosition.lineNumber ?? "?";
            const iconId = `vehicle-${routeColor.replace("#", "")}-${lineNum}`;

            if (!this.vehicleIcons.has(iconId)) {
                this.layerManager.addImage(iconId, createVehicleIcon(routeColor, lineNum));
                this.vehicleIcons.add(iconId);
            }

            // Add marker feature
            markerFeatures.push({
                type: "Feature",
                properties: {
                    tripId: smoothedPosition.tripId,
                    lineNumber: smoothedPosition.lineNumber,
                    destination: smoothedPosition.destination,
                    status: smoothedPosition.status,
                    delayMinutes: smoothedPosition.delayMinutes,
                    bearing: smoothedPosition.renderedBearing,
                    color: routeColor,
                    iconId,
                    currentStopName: smoothedPosition.currentStop?.stop_name ?? null,
                    nextStopName: smoothedPosition.nextStop?.stop_name ?? null,
                },
                geometry: { type: "Point", coordinates: [smoothedPosition.renderedLon, smoothedPosition.renderedLat] },
            });

            // Generate 3D model features and debug visualization
            const linearizedRoute = this.linearizedRoutes.get(routeId);
            const isTracked = targetPosition.tripId === this.trackedTripId;

            // Determine if we should show debug for this vehicle
            const showDebugForThis = this.debugOptions.showDebugSegments &&
                (!this.debugOptions.showDebugOnlyTracked || isTracked);

            // Generate 3D models and/or debug visualization
            if (this.debugOptions.show3DModels || showDebugForThis) {
                const { modelFeatures: segmentFeatures, debugFeatures: segDebugFeatures } = this.generateModelFeatures(
                    smoothedPosition,
                    linearizedRoute,
                    routeColor,
                    vehicleModel,
                    segmentDistances,
                    showDebugForThis
                );
                // Only add 3D model features if enabled
                if (this.debugOptions.show3DModels) {
                    modelFeatures.push(...segmentFeatures);
                }
                // Only add debug features if requested for this vehicle
                if (showDebugForThis) {
                    debugFeatures.push(...segDebugFeatures);
                }
            }
        }

        // Cleanup old positions
        for (const tripId of this.smoothedPositions.keys()) {
            if (!activeTripIds.has(tripId)) {
                this.smoothedPositions.delete(tripId);
            }
        }

        // Check if tracked vehicle still exists
        if (this.trackedTripId && !this.smoothedPositions.has(this.trackedTripId)) {
            this.onTrackedVehicleLost?.();
        }

        // Update all layers together
        this.layerManager.updateVehicles(markerFeatures);
        this.layerManager.updateVehicleModels(modelFeatures);
        this.layerManager.updateDebugSegments(debugFeatures);
    }

    /**
     * Generate 3D model features for a vehicle using linearized route
     * Uses the linear position calculated during vehicle position interpolation (not proximity search)
     */
    private generateModelFeatures(
        smoothedPosition: SmoothedVehiclePosition,
        linearizedRoute: LinearizedRoute | undefined,
        routeColor: string,
        vehicleModel: ReturnType<typeof getAugsburgVehicleModel>,
        segmentDistances: ReturnType<typeof calculateSegmentDistances>,
        showDebug = false
    ): { modelFeatures: GeoJSON.Feature[]; debugFeatures: GeoJSON.Feature[] } {
        const modelFeatures: GeoJSON.Feature[] = [];
        const debugFeatures: GeoJSON.Feature[] = [];

        // If no linearized route or no linear position info, don't render 3D model
        if (!linearizedRoute || smoothedPosition.renderedLinearPosition === undefined) {
            return { modelFeatures: [], debugFeatures: [] };
        }

        // Use the linear position that was calculated from the stop-to-stop interpolation
        // This is NOT a proximity search - it comes from the actual route following logic
        const linearPosition = smoothedPosition.renderedLinearPosition;

        // Get all distances behind the vehicle for 3D model segments
        const allDistances: number[] = [];
        for (const segInfo of segmentDistances) {
            allDistances.push(segInfo.frontDistance, segInfo.rearDistance);
        }

        // Get positions along the route behind the vehicle
        const positions = getPositionsBehindOnRoute(linearizedRoute, linearPosition, allDistances);

        // Generate 3D model polygons
        for (let i = 0; i < segmentDistances.length; i++) {
            const segInfo = segmentDistances[i];
            const frontPos = positions[i * 2];
            const rearPos = positions[i * 2 + 1];

            const polygon = this.createSegmentPolygon(
                frontPos.lon, frontPos.lat,
                rearPos.lon, rearPos.lat,
                vehicleModel.width
            );

            if (polygon.length > 0) {
                modelFeatures.push({
                    type: "Feature",
                    properties: {
                        color: routeColor,
                        tripId: smoothedPosition.tripId,
                        carIndex: segInfo.index,
                        height: segInfo.segment.height,
                    },
                    geometry: { type: "Polygon", coordinates: [polygon] },
                });
            }
        }

        // Generate debug segment visualization if this is the tracked vehicle
        if (showDebug) {
            // Find current segment from linear position
            const segmentIndex = this.findSegmentIndexFromLinearPosition(linearizedRoute, linearPosition);
            const segDebug = getDebugSegmentFeatures(linearizedRoute, segmentIndex, 5, 5);
            debugFeatures.push(...segDebug);
        }

        return { modelFeatures, debugFeatures };
    }

    /**
     * Find segment index from a linear position along the route
     */
    private findSegmentIndexFromLinearPosition(route: LinearizedRoute, linearPosition: number): number {
        for (let i = 0; i < route.distances.length - 1; i++) {
            if (linearPosition <= route.distances[i + 1]) {
                return i;
            }
        }
        return route.coords.length - 2;
    }

    /**
     * Create a polygon for a tram segment
     */
    private createSegmentPolygon(
        frontLon: number,
        frontLat: number,
        rearLon: number,
        rearLat: number,
        width: number
    ): number[][] {
        const metersPerDegreeLat = 111320;
        const metersPerDegreeLon = 111320 * Math.cos((frontLat * Math.PI) / 180);
        const dx = (frontLon - rearLon) * metersPerDegreeLon;
        const dy = (frontLat - rearLat) * metersPerDegreeLat;
        const length = Math.sqrt(dx * dx + dy * dy);
        if (length < 0.1) return [];

        const dirX = dx / length;
        const dirY = dy / length;
        const perpX = dirY;
        const perpY = -dirX;
        const halfWidth = width / 2;

        const corners = [
            [frontLon + (perpX * halfWidth) / metersPerDegreeLon, frontLat + (perpY * halfWidth) / metersPerDegreeLat],
            [frontLon - (perpX * halfWidth) / metersPerDegreeLon, frontLat - (perpY * halfWidth) / metersPerDegreeLat],
            [rearLon - (perpX * halfWidth) / metersPerDegreeLon, rearLat - (perpY * halfWidth) / metersPerDegreeLat],
            [rearLon + (perpX * halfWidth) / metersPerDegreeLon, rearLat + (perpY * halfWidth) / metersPerDegreeLat],
        ];
        corners.push(corners[0]);
        return corners;
    }

    /**
     * Cleanup resources
     */
    dispose(): void {
        this.stopAnimation();
        this.vehicleIcons.clear();
        this.smoothedPositions.clear();
        this.linearizedRoutes.clear();
    }
}
