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
    findPositionOnRoute,
    getDebugSegmentFeatures,
    getPositionAtDistance,
    getPositionsBehindOnRoute,
    linearizeRoute,
    updateSmoothedPosition,
    type LinearizedRoute,
    type SmoothedVehiclePosition,
    type VehiclePosition,
} from "./vehicleUtils";
import { featureManager, type VehicleRenderContext, type RenderPosition } from "./features";

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

    // Current vehicles data - updated via setVehicles() so animation loop uses latest data
    private currentVehicles: RouteVehicles[] = [];

    // Current simulated time - updated via setSimulatedTime()
    private simulatedTime: Date = new Date();

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
     * Update the vehicles data used by the animation loop
     * This should be called whenever vehicles prop changes
     */
    setVehicles(vehicles: RouteVehicles[]): void {
        this.currentVehicles = vehicles;
    }

    /**
     * Update the simulated time used for vehicle position calculations
     * This should be called whenever the simulated time changes
     */
    setSimulatedTime(time: Date): void {
        this.simulatedTime = time;
    }

    /**
     * Start the vehicle animation loop
     * Uses this.currentVehicles which should be updated via setVehicles()
     */
    startAnimation(): void {
        if (this.animationId) return;

        this.updatePositions(this.currentVehicles, ANIMATION_INTERVAL);

        const animate = (timestamp: number) => {
            const deltaMs = this.lastAnimationTime > 0 ? timestamp - this.lastAnimationTime : ANIMATION_INTERVAL;
            if (deltaMs >= ANIMATION_INTERVAL) {
                this.lastAnimationTime = timestamp;
                // Use this.currentVehicles so animation always uses latest data
                this.updatePositions(this.currentVehicles, deltaMs);
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
        // Use simulated time for position calculations
        const now = this.simulatedTime;
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
        }

        // Collect vehicle context for feature processing
        const vehicleContexts: VehicleRenderContext[] = [];
        for (const { position: targetPosition, routeId } of allPositions) {
            if (!activeTripIds.has(targetPosition.tripId)) continue;

            const smoothedPosition = this.smoothedPositions.get(targetPosition.tripId);
            if (!smoothedPosition) continue;

            // Get linear position from rendered position
            const linearizedRoute = this.linearizedRoutes.get(routeId);
            if (!linearizedRoute) continue;

            const routePosition = findPositionOnRoute(
                linearizedRoute,
                smoothedPosition.renderedLon,
                smoothedPosition.renderedLat
            );

            vehicleContexts.push({
                tripId: targetPosition.tripId,
                routeId,
                linearPosition: routePosition.linearPosition,
                smoothedPosition,
            });
        }

        // Process render positions through feature pipeline
        const renderPositions = this.processRenderPositions(vehicleContexts);

        // Now generate features using processed render positions
        for (const { position: targetPosition, routeId, routeColor } of allPositions) {
            if (!activeTripIds.has(targetPosition.tripId)) continue;

            const smoothedPosition = this.smoothedPositions.get(targetPosition.tripId);
            if (!smoothedPosition) continue;

            // Get processed render position (or fall back to smoothed)
            const renderPos = renderPositions.get(targetPosition.tripId) ?? {
                lon: smoothedPosition.renderedLon,
                lat: smoothedPosition.renderedLat,
                bearing: smoothedPosition.renderedBearing,
            };

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
                    bearing: renderPos.bearing,
                    color: routeColor,
                    iconId,
                    currentStopName: smoothedPosition.currentStop?.stop_name ?? null,
                    nextStopName: smoothedPosition.nextStop?.stop_name ?? null,
                },
                geometry: { type: "Point", coordinates: [renderPos.lon, renderPos.lat] },
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
                    renderPos,
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
     * The 3D model position is derived from the render position (after collision avoidance)
     */
    private generateModelFeatures(
        smoothedPosition: SmoothedVehiclePosition,
        renderPos: { lon: number; lat: number; bearing: number },
        linearizedRoute: LinearizedRoute | undefined,
        routeColor: string,
        vehicleModel: ReturnType<typeof getAugsburgVehicleModel>,
        segmentDistances: ReturnType<typeof calculateSegmentDistances>,
        showDebug = false
    ): { modelFeatures: GeoJSON.Feature[]; debugFeatures: GeoJSON.Feature[] } {
        const modelFeatures: GeoJSON.Feature[] = [];
        const debugFeatures: GeoJSON.Feature[] = [];

        // If no linearized route, don't render 3D model
        if (!linearizedRoute) {
            return { modelFeatures: [], debugFeatures: [] };
        }

        // Project the render position onto the route to get linear position
        const routePosition = findPositionOnRoute(
            linearizedRoute,
            renderPos.lon,
            renderPos.lat
        );
        const linearPosition = routePosition.linearPosition;

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
            // Use segment index from the route projection (same source as 3D model)
            const segDebug = getDebugSegmentFeatures(linearizedRoute, routePosition.segmentIndex, 5, 5);
            debugFeatures.push(...segDebug);
        }

        return { modelFeatures, debugFeatures };
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
     * Process render positions through feature pipeline
     * Returns a map of tripId -> {lon, lat, bearing} for rendering
     */
    private processRenderPositions(vehicleContexts: VehicleRenderContext[]): globalThis.Map<string, RenderPosition> {
        const renderPositions = new globalThis.Map<string, RenderPosition>();

        if (vehicleContexts.length === 0) return renderPositions;

        // Initialize render positions from smoothed positions
        for (const vehicle of vehicleContexts) {
            renderPositions.set(vehicle.tripId, {
                lon: vehicle.smoothedPosition.renderedLon,
                lat: vehicle.smoothedPosition.renderedLat,
                bearing: vehicle.smoothedPosition.renderedBearing,
            });
        }

        // Process through all enabled features
        featureManager.processRenderPositions(vehicleContexts, renderPositions, this.linearizedRoutes);

        return renderPositions;
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
