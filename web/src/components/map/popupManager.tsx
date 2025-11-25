import maplibregl from "maplibre-gl";
import { createRoot, Root } from "react-dom/client";
import React from "react";
import { StationPopup } from "./StationPopup";
import { PlatformPopup } from "./PlatformPopup";
import { Platform, Station } from "./types";

export class PopupManager {
    private map: maplibregl.Map;
    private stationsMap: { [stationId: string]: Station };

    constructor(map: maplibregl.Map, stationsMap: { [stationId: string]: Station }) {
        this.map = map;
        this.stationsMap = stationsMap;
    }

    showStationPopup(stationId: string, stationName: string, coordinates: [number, number]) {
        // Get all platforms for this station
        const station = this.stationsMap[stationId];

        // Create a div for React component
        const popupNode = document.createElement("div");
        const root = createRoot(popupNode);

        // Show station info popup
        const popup = new maplibregl.Popup({
            maxWidth: "400px",
            closeButton: false,
            closeOnClick: false
        })
            .setLngLat(coordinates)
            .setDOMContent(popupNode)
            .addTo(this.map);

        // Function to show platform popup from station
        const showPlatformPopup = (platform: Platform) => {
            if (platform?.coord && platform.coord.length === 2) {
                // Close current popup
                popup.remove();

                // EFA API returns [lat, lon] but MapLibre expects [lon, lat]
                const platformCoord: [number, number] = [platform.coord[1], platform.coord[0]];

                this.showPlatformPopupInternal(
                    platform,
                    stationName,
                    platformCoord,
                    stationId,
                    coordinates
                );
            }
        };

        // Render station popup with React
        root.render(
            <StationPopup
                stationName={stationName}
                platforms={station?.platforms || []}
                onPlatformClick={showPlatformPopup}
                onClose={() => popup.remove()}
            />
        );
    }

    private showPlatformPopupInternal(
        platform: Platform,
        stationName: string,
        platformCoord: [number, number],
        stationId: string,
        stationCoordinates: [number, number]
    ) {
        // Create platform popup
        const platformPopupNode = document.createElement("div");
        const platformRoot = createRoot(platformPopupNode);

        const platformPopup = new maplibregl.Popup({
            maxWidth: "350px",
            closeButton: false,
            closeOnClick: false
        })
            .setLngLat(platformCoord)
            .setDOMContent(platformPopupNode)
            .addTo(this.map);

        platformRoot.render(
            <PlatformPopup
                platformName={platform.name}
                stationName={stationName}
                platformId={platform.id}
                osmId={platform.osm_id}
                onClose={() => platformPopup.remove()}
                onStationClick={() => {
                    platformPopup.remove();
                    // Find station coordinates
                    const station = this.stationsMap[stationId];
                    const stationCoord =
                        station.coord && station.coord.length === 2
                            ? ([station.coord[1], station.coord[0]] as [number, number])
                            : stationCoordinates;
                    this.showStationPopup(stationId, stationName, stationCoord);
                }}
            />
        );
    }

    showPlatformPopupFromMap(
        platformName: string,
        stationName: string,
        platformId: string,
        stationId: string,
        osmId: string | undefined,
        coordinates: [number, number]
    ) {
        // Get station to find its coordinates
        const station = this.stationsMap[stationId];

        // Create a div for React component
        const popupNode = document.createElement("div");
        const root = createRoot(popupNode);

        // Show platform info popup
        const popup = new maplibregl.Popup({
            maxWidth: "350px",
            closeButton: false,
            closeOnClick: false
        })
            .setLngLat(coordinates)
            .setDOMContent(popupNode)
            .addTo(this.map);

        root.render(
            <PlatformPopup
                platformName={platformName}
                stationName={stationName}
                platformId={platformId}
                osmId={osmId}
                onClose={() => popup.remove()}
                onStationClick={() => {
                    popup.remove();
                    // Find station coordinates
                    let stationCoord: [number, number];
                    if (station.coord && station.coord.length === 2) {
                        stationCoord = [station.coord[1], station.coord[0]];
                    } else if (station.platforms.length > 0) {
                        // Calculate average position from platforms
                        let avgLon = 0;
                        let avgLat = 0;
                        let count = 0;
                        station.platforms.forEach((p: any) => {
                            if (p.coord && p.coord.length === 2) {
                                avgLon += p.coord[1];
                                avgLat += p.coord[0];
                                count++;
                            }
                        });
                        stationCoord = count > 0 ? [avgLon / count, avgLat / count] : coordinates;
                    } else {
                        stationCoord = coordinates;
                    }
                    this.showStationPopup(stationId, stationName, stationCoord);
                }}
            />
        );
    }
}
