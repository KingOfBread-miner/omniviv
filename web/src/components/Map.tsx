import maplibregl from "maplibre-gl";
import "maplibre-gl/dist/maplibre-gl.css";
import { useEffect, useRef } from "react";

interface MapProps {
    className?: string;
}

export default function Map({ className }: MapProps) {
    const mapContainer = useRef<HTMLDivElement>(null);
    const map = useRef<maplibregl.Map | null>(null);

    useEffect(() => {
        if (map.current || !mapContainer.current) return;

        // Initialize map
        map.current = new maplibregl.Map({
            container: mapContainer.current,
            style: "http://localhost:8080/styles/basic-preview/style.json",
            center: [10.898, 48.371], // Augsburg coordinates
            zoom: 16,
            pitch: 30, // Tilt the map for 3D effect
            bearing: 0, // Initial rotation
            antialias: true // Smooth 3D rendering
        });

        // Add navigation controls
        map.current.addControl(new maplibregl.NavigationControl(), "top-right");

        // Add scale control
        map.current.addControl(new maplibregl.ScaleControl(), "bottom-left");

        // Load tram data when map is ready
        map.current.on("load", async () => {
            if (!map.current) return;

            // Add 3D buildings
            map.current.addLayer({
                id: "3d-buildings",
                source: "openmaptiles",
                "source-layer": "building",
                type: "fill-extrusion",
                minzoom: 14,
                paint: {
                    "fill-extrusion-color": "#aaa",
                    "fill-extrusion-height": [
                        "interpolate",
                        ["linear"],
                        ["zoom"],
                        15,
                        0,
                        15.05,
                        ["get", "render_height"]
                    ],
                    "fill-extrusion-base": [
                        "interpolate",
                        ["linear"],
                        ["zoom"],
                        15,
                        0,
                        15.05,
                        ["get", "render_min_height"]
                    ],
                    "fill-extrusion-opacity": 0.6
                }
            });

            // Fetch line geometries from the API endpoint
            const lineRefs = ["1", "2", "3", "4", "6"]; // Only include lines 1-6

            try {
                const lineGeometriesResponse = await fetch(
                    "http://localhost:3000/api/lines/geometries",
                    {
                        method: "POST",
                        headers: {
                            "Content-Type": "application/json"
                        },
                        body: JSON.stringify({ line_refs: lineRefs })
                    }
                );

                const lineGeometries = await lineGeometriesResponse.json();
                console.log(`Received ${lineGeometries.length} line geometries from API`);

                // Add each tram line as a separate layer with its own color
                lineGeometries.forEach((lineData: any) => {
                    if (lineData.segments.length > 0) {
                        // Add source for this line with multiple line strings
                        map.current!.addSource(`tram-line-${lineData.line_ref}`, {
                            type: "geojson",
                            data: {
                                type: "Feature",
                                properties: {
                                    ref: lineData.line_ref,
                                    color: lineData.color
                                },
                                geometry: {
                                    type: "MultiLineString",
                                    coordinates: lineData.segments
                                }
                            }
                        });

                        // Add layer for this line
                        map.current!.addLayer(
                            {
                                id: `tram-line-${lineData.line_ref}`,
                                type: "line",
                                source: `tram-line-${lineData.line_ref}`,
                                paint: {
                                    "line-color": lineData.color,
                                    "line-width": 4,
                                    "line-opacity": 0.8
                                },
                                layout: {
                                    "line-cap": "round",
                                    "line-join": "round"
                                }
                            },
                            "3d-buildings"
                        );

                        const totalPoints = lineData.segments.reduce(
                            (sum: number, seg: any) => sum + seg.length,
                            0
                        );
                        console.log(
                            `Added line ${lineData.line_ref} with ${lineData.segments.length} segments and ${totalPoints} points in color ${lineData.color}`
                        );
                    }
                });
            } catch (error) {
                console.error("Error fetching line geometries:", error);
            }

            // Fetch tram stations (OSM) from the API endpoint
            try {
                const stationsResponse = await fetch("http://localhost:3000/api/stations");
                const stations = await stationsResponse.json();
                console.log(`Received ${stations.length} OSM tram stations from API`);

                // Add tram stations as points
                map.current.addSource("tram-stations", {
                    type: "geojson",
                    data: {
                        type: "FeatureCollection",
                        features: stations
                            .filter((station: any) => station.name) // Only show stations with names
                            .map((station: any) => ({
                                type: "Feature",
                                geometry: {
                                    type: "Point",
                                    coordinates: [station.lon, station.lat]
                                },
                                properties: {
                                    id: station.id,
                                    name: station.name,
                                    tags: station.tags
                                }
                            }))
                    }
                });

                // Add station circles
                map.current.addLayer({
                    id: "tram-stations-circle",
                    type: "circle",
                    source: "tram-stations",
                    paint: {
                        "circle-radius": 6,
                        "circle-color": "#e3000f",
                        "circle-stroke-width": 2,
                        "circle-stroke-color": "#ffffff"
                    }
                });

                // Add station labels
                map.current.addLayer({
                    id: "tram-stations-label",
                    type: "symbol",
                    source: "tram-stations",
                    layout: {
                        "text-field": ["get", "name"],
                        "text-font": ["Open Sans Regular"],
                        "text-offset": [0, 1.5],
                        "text-anchor": "top",
                        "text-size": 12
                    },
                    paint: {
                        "text-color": "#000000",
                        "text-halo-color": "#ffffff",
                        "text-halo-width": 2
                    }
                });

                // Add click handler for stations to show info
                map.current.on("click", "tram-stations-circle", (e) => {
                    if (!e.features || !e.features[0]) return;

                    const coordinates = (e.features[0].geometry as any).coordinates.slice();
                    const name = e.features[0].properties?.name;
                    const stationId = e.features[0].properties?.id;

                    // Show station info popup
                    new maplibregl.Popup({ maxWidth: "350px" })
                        .setLngLat(coordinates)
                        .setHTML(
                            `<div class="p-3">
                                <h3 class="font-bold text-lg mb-2">${name}</h3>
                                <p class="text-sm text-gray-600">OSM ID: ${stationId}</p>
                            </div>`
                        )
                        .addTo(map.current!);
                });

                // Change cursor on hover
                map.current.on("mouseenter", "tram-stations-circle", () => {
                    if (map.current) map.current.getCanvas().style.cursor = "pointer";
                });

                map.current.on("mouseleave", "tram-stations-circle", () => {
                    if (map.current) map.current.getCanvas().style.cursor = "";
                });
            } catch (error) {
                console.error("Error loading tram stations:", error);
            }
        });

        // Cleanup
        return () => {
            map.current?.remove();
            map.current = null;
        };
    }, []);

    return (
        <div ref={mapContainer} className={className} style={{ width: "100%", height: "100%" }} />
    );
}
