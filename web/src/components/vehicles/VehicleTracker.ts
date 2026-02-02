/**
 * Handles vehicle tracking mode with camera follow and custom interactions
 */

import type maplibregl from "maplibre-gl";
import type { SmoothedVehiclePosition } from "./vehicleUtils";

export interface TrackingInfo {
    lineNumber: string;
    destination: string;
    nextStopName: string | null;
    progress: number;
    secondsToNextStop: number | null;
    status: string;
    color: string;
}

export interface TrackingCallbacks {
    onTrackingInfoUpdate: (info: TrackingInfo) => void;
    onTrackingStop: () => void;
    getSmoothedPosition: (tripId: string) => SmoothedVehiclePosition | undefined;
    getRouteColor: (lineNumber: string) => string;
}

const MIN_TRACKING_ZOOM = 16;

export class VehicleTracker {
    private map: maplibregl.Map;
    private callbacks: TrackingCallbacks;
    private tripId: string | null = null;
    private simulatedTime: Date = new Date();

    // Animation state
    private trackingAnimationId: number | null = null;
    private isZoomingIn = false;

    // Drag state
    private isRightDragging = false;
    private isLeftDragging = false;
    private lastMouseX = 0;
    private lastMouseY = 0;

    // Bound event handlers
    private boundHandleWheel: ((e: WheelEvent) => void) | null = null;
    private boundHandleMouseDown: ((e: MouseEvent) => void) | null = null;
    private boundHandleMouseMove: ((e: MouseEvent) => void) | null = null;
    private boundHandleMouseUp: ((e: MouseEvent) => void) | null = null;
    private boundHandleContextMenu: ((e: MouseEvent) => void) | null = null;

    constructor(map: maplibregl.Map, callbacks: TrackingCallbacks) {
        this.map = map;
        this.callbacks = callbacks;
    }

    /**
     * Start tracking a vehicle
     */
    startTracking(tripId: string): void {
        this.tripId = tripId;
        this.resetDragState();

        // Zoom in if needed
        if (this.map.getZoom() < MIN_TRACKING_ZOOM) {
            const trackedPosition = this.callbacks.getSmoothedPosition(tripId);
            if (trackedPosition) {
                this.isZoomingIn = true;
                this.map.flyTo({
                    center: [trackedPosition.renderedLon, trackedPosition.renderedLat],
                    zoom: MIN_TRACKING_ZOOM,
                    duration: 1000,
                });
                this.map.once("moveend", () => {
                    this.isZoomingIn = false;
                });
            }
        }

        // Disable native handlers
        this.map.dragPan.disable();
        this.map.scrollZoom.disable();
        this.map.dragRotate.disable();

        // Set up event listeners
        this.setupEventListeners();

        // Start tracking animation
        this.startTrackingAnimation();
    }

    /**
     * Stop tracking the current vehicle
     */
    stopTracking(): void {
        this.cleanupEventListeners();
        this.tripId = null;

        // Re-enable native handlers
        this.map.dragPan.enable();
        this.map.scrollZoom.enable();
        this.map.dragRotate.enable();
    }

    /**
     * Check if currently tracking a vehicle
     */
    isTracking(): boolean {
        return this.tripId !== null;
    }

    /**
     * Get the currently tracked trip ID
     */
    getTrackedTripId(): string | null {
        return this.tripId;
    }

    /**
     * Update the simulated time used for countdown calculations
     */
    setSimulatedTime(time: Date): void {
        this.simulatedTime = time;
    }

    /**
     * Cleanup resources
     */
    dispose(): void {
        this.cleanupEventListeners();
        this.tripId = null;
    }

    private resetDragState(): void {
        this.isZoomingIn = false;
        this.isLeftDragging = false;
        this.isRightDragging = false;
        this.lastMouseX = 0;
        this.lastMouseY = 0;
    }

    private setupEventListeners(): void {
        this.boundHandleWheel = this.handleWheel.bind(this);
        this.boundHandleMouseDown = this.handleMouseDown.bind(this);
        this.boundHandleMouseMove = this.handleMouseMove.bind(this);
        this.boundHandleMouseUp = this.handleMouseUp.bind(this);
        this.boundHandleContextMenu = (e: MouseEvent) => e.preventDefault();

        const canvas = this.map.getCanvas();
        canvas.addEventListener("wheel", this.boundHandleWheel, { passive: false });
        canvas.addEventListener("mousedown", this.boundHandleMouseDown);
        canvas.addEventListener("contextmenu", this.boundHandleContextMenu);
        window.addEventListener("mousemove", this.boundHandleMouseMove);
        window.addEventListener("mouseup", this.boundHandleMouseUp);
    }

    private cleanupEventListeners(): void {
        if (this.trackingAnimationId) {
            cancelAnimationFrame(this.trackingAnimationId);
            this.trackingAnimationId = null;
        }

        const canvas = this.map.getCanvas();
        if (this.boundHandleWheel) canvas.removeEventListener("wheel", this.boundHandleWheel);
        if (this.boundHandleMouseDown) canvas.removeEventListener("mousedown", this.boundHandleMouseDown);
        if (this.boundHandleContextMenu) canvas.removeEventListener("contextmenu", this.boundHandleContextMenu);
        if (this.boundHandleMouseMove) window.removeEventListener("mousemove", this.boundHandleMouseMove);
        if (this.boundHandleMouseUp) window.removeEventListener("mouseup", this.boundHandleMouseUp);

        this.boundHandleWheel = null;
        this.boundHandleMouseDown = null;
        this.boundHandleMouseMove = null;
        this.boundHandleMouseUp = null;
        this.boundHandleContextMenu = null;

        this.resetDragState();
    }

    private handleWheel(e: WheelEvent): void {
        e.preventDefault();
        if (!this.tripId) return;

        const trackedPosition = this.callbacks.getSmoothedPosition(this.tripId);
        if (!trackedPosition) return;

        const currentZoom = this.map.getZoom();
        const zoomDelta = -e.deltaY * 0.002;
        const newZoom = Math.max(10, Math.min(20, currentZoom + zoomDelta));
        this.map.setZoom(newZoom);
    }

    private handleMouseDown(e: MouseEvent): void {
        this.lastMouseX = e.clientX;
        this.lastMouseY = e.clientY;

        if (e.button === 2) {
            this.isRightDragging = true;
            e.preventDefault();
        } else if (e.button === 0) {
            this.isLeftDragging = true;
        }
    }

    private handleMouseMove(e: MouseEvent): void {
        const deltaX = e.clientX - this.lastMouseX;
        const deltaY = e.clientY - this.lastMouseY;

        if (this.isLeftDragging && (Math.abs(deltaX) > 3 || Math.abs(deltaY) > 3)) {
            this.isLeftDragging = false;
            this.map.dragPan.enable();
            this.map.scrollZoom.enable();
            this.map.dragRotate.enable();

            const canvas = this.map.getCanvas();
            const syntheticEvent = new MouseEvent("mousedown", {
                clientX: this.lastMouseX,
                clientY: this.lastMouseY,
                button: 0,
                bubbles: true,
            });
            canvas.dispatchEvent(syntheticEvent);
            this.callbacks.onTrackingStop();
            return;
        }

        if (this.isRightDragging) {
            this.lastMouseX = e.clientX;
            this.lastMouseY = e.clientY;
            const currentBearing = this.map.getBearing();
            const currentPitch = this.map.getPitch();
            this.map.setBearing(currentBearing + deltaX * 0.5);
            this.map.setPitch(Math.max(0, Math.min(85, currentPitch - deltaY * 0.5)));
        }
    }

    private handleMouseUp(): void {
        this.isRightDragging = false;
        this.isLeftDragging = false;
    }

    private startTrackingAnimation(): void {
        const trackVehicle = () => {
            if (!this.tripId) return;

            const trackedPosition = this.callbacks.getSmoothedPosition(this.tripId);
            if (trackedPosition) {
                if (!this.isZoomingIn) {
                    this.map.setCenter([trackedPosition.renderedLon, trackedPosition.renderedLat]);
                }

                let secondsToNextStop: number | null = null;
                if (trackedPosition.nextStop) {
                    const arrivalTimeStr = trackedPosition.nextStop.arrival_time_estimated || trackedPosition.nextStop.arrival_time;
                    if (arrivalTimeStr) {
                        const arrivalTime = new Date(arrivalTimeStr).getTime();
                        secondsToNextStop = Math.max(0, Math.round((arrivalTime - this.simulatedTime.getTime()) / 1000));
                    }
                }

                const routeColor = this.callbacks.getRouteColor(trackedPosition.lineNumber);

                this.callbacks.onTrackingInfoUpdate({
                    lineNumber: trackedPosition.lineNumber,
                    destination: trackedPosition.destination,
                    nextStopName: trackedPosition.nextStop?.stop_name ?? null,
                    progress: trackedPosition.progress,
                    secondsToNextStop,
                    status: trackedPosition.status,
                    color: routeColor,
                });
            }
            this.trackingAnimationId = requestAnimationFrame(trackVehicle);
        };

        this.trackingAnimationId = requestAnimationFrame(trackVehicle);
    }
}
