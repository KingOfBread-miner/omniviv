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
    findPositionsAlongTrack,
    updateSmoothedPosition,
    type SmoothedVehiclePosition,
    type VehiclePosition,
} from "./vehicleUtils";

const ANIMATION_INTERVAL = 50;

interface SegmentPosition {
    frontLon: number;
    frontLat: number;
    rearLon: number;
    rearLat: number;
}

export class VehicleRenderer {
    private layerManager: MapLayerManager;
    private routeColors: globalThis.Map<string, string>;
    private routeGeometries: globalThis.Map<number, number[][][]>;
    private smoothedPositions = new globalThis.Map<string, SmoothedVehiclePosition>();
    private modelSegmentPositions = new globalThis.Map<string, SegmentPosition[]>();
    private vehicleIcons = new Set<string>();
    private animationId: number | null = null;
    private lastAnimationTime = 0;

    private onTrackedVehicleLost?: () => void;
    private trackedTripId: string | null = null;

    constructor(
        layerManager: MapLayerManager,
        routeColors: globalThis.Map<string, string>,
        routeGeometries: globalThis.Map<number, number[][][]>
    ) {
        this.layerManager = layerManager;
        this.routeColors = routeColors;
        this.routeGeometries = routeGeometries;
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
        this.smoothedPositions.clear();
        this.modelSegmentPositions.clear();
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

            // Generate 3D model features using same smoothed position
            const routeGeometry = this.routeGeometries.get(routeId) ?? [];
            const segmentFeatures = this.generateModelFeatures(
                targetPosition.tripId,
                smoothedPosition,
                routeGeometry,
                routeColor,
                vehicleModel,
                segmentDistances
            );
            modelFeatures.push(...segmentFeatures);
        }

        // Cleanup old positions
        for (const tripId of this.smoothedPositions.keys()) {
            if (!activeTripIds.has(tripId)) {
                this.smoothedPositions.delete(tripId);
                this.modelSegmentPositions.delete(tripId);
            }
        }

        // Check if tracked vehicle still exists
        if (this.trackedTripId && !this.smoothedPositions.has(this.trackedTripId)) {
            this.onTrackedVehicleLost?.();
        }

        // Update both layers together
        this.layerManager.updateVehicles(markerFeatures);
        this.layerManager.updateVehicleModels(modelFeatures);
    }

    /**
     * Generate 3D model features for a vehicle
     */
    private generateModelFeatures(
        tripId: string,
        smoothedPosition: SmoothedVehiclePosition,
        routeGeometry: number[][][],
        routeColor: string,
        vehicleModel: ReturnType<typeof getAugsburgVehicleModel>,
        segmentDistances: ReturnType<typeof calculateSegmentDistances>
    ): GeoJSON.Feature[] {
        const features: GeoJSON.Feature[] = [];

        const lon = smoothedPosition.renderedLon;
        const lat = smoothedPosition.renderedLat;
        const bearing = smoothedPosition.renderedBearing;

        const allDistances: number[] = [];
        for (const segInfo of segmentDistances) {
            allDistances.push(segInfo.frontDistance, segInfo.rearDistance);
        }

        const allTrackPositions = findPositionsAlongTrack(lon, lat, allDistances, bearing, routeGeometry);

        const newPositions: SegmentPosition[] = [];
        let allValid = true;
        const lastValidPositions = this.modelSegmentPositions.get(tripId) ?? [];

        for (let i = 0; i < segmentDistances.length; i++) {
            const segInfo = segmentDistances[i];
            const frontPos = allTrackPositions[i * 2];
            const rearPos = allTrackPositions[i * 2 + 1];

            const actualLength = this.distanceMeters(frontPos.lon, frontPos.lat, rearPos.lon, rearPos.lat);
            const lengthRatio = actualLength / segInfo.segment.length;

            if (lengthRatio < 0.5 || lengthRatio > 1.5) {
                allValid = false;
                break;
            }

            if (lastValidPositions.length > 0 && lastValidPositions[i]) {
                const lastFront = lastValidPositions[i];
                const frontMovement = this.distanceMeters(lastFront.frontLon, lastFront.frontLat, frontPos.lon, frontPos.lat);
                if (frontMovement > 20) {
                    allValid = false;
                    break;
                }
            }

            newPositions.push({
                frontLon: frontPos.lon,
                frontLat: frontPos.lat,
                rearLon: rearPos.lon,
                rearLat: rearPos.lat,
            });
        }

        const positionsToUse = (allValid && newPositions.length === segmentDistances.length) ? newPositions : lastValidPositions;
        if (allValid && newPositions.length === segmentDistances.length) {
            this.modelSegmentPositions.set(tripId, newPositions);
        }

        for (let i = 0; i < segmentDistances.length && i < positionsToUse.length; i++) {
            const segInfo = segmentDistances[i];
            const pos = positionsToUse[i];
            if (!pos) continue;

            const polygon = this.createSegmentPolygon(pos.frontLon, pos.frontLat, pos.rearLon, pos.rearLat, vehicleModel.width);
            if (polygon.length > 0) {
                features.push({
                    type: "Feature",
                    properties: {
                        color: routeColor,
                        tripId,
                        carIndex: segInfo.index,
                        height: segInfo.segment.height,
                    },
                    geometry: { type: "Polygon", coordinates: [polygon] },
                });
            }
        }

        return features;
    }

    /**
     * Calculate distance in meters between two coordinates
     */
    private distanceMeters(lon1: number, lat1: number, lon2: number, lat2: number): number {
        const R = 6371000;
        const phi1 = (lat1 * Math.PI) / 180;
        const phi2 = (lat2 * Math.PI) / 180;
        const dPhi = ((lat2 - lat1) * Math.PI) / 180;
        const dLambda = ((lon2 - lon1) * Math.PI) / 180;
        const a = Math.sin(dPhi / 2) ** 2 + Math.cos(phi1) * Math.cos(phi2) * Math.sin(dLambda / 2) ** 2;
        return R * 2 * Math.atan2(Math.sqrt(a), Math.sqrt(1 - a));
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
        this.modelSegmentPositions.clear();
    }
}
