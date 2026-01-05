import { useState, useRef } from "react";
import { Crosshair, MapPin } from "lucide-react";
import { Button } from "./ui/button";
import { Input } from "./ui/input";
import { Popover, PopoverContent, PopoverAnchor } from "./ui/popover";
import type { Station } from "../api";

interface Location {
    name: string;
    lat: number;
    lon: number;
}

interface LocationInputProps {
    label: string;
    placeholder?: string;
    stations: Station[];
    value: Location | null;
    onChange: (location: Location | null) => void;
    onUseCurrentLocation?: () => void;
    onPickOnMap?: () => void;
    isLocating?: boolean;
}

function LocationInput({
    label,
    placeholder = "Search station...",
    stations,
    value,
    onChange,
    onUseCurrentLocation,
    onPickOnMap,
    isLocating,
}: LocationInputProps) {
    const [query, setQuery] = useState(value?.name ?? "");
    const [isOpen, setIsOpen] = useState(false);
    const inputRef = useRef<HTMLInputElement>(null);

    const filteredStations = query.length > 0 && !value
        ? stations.filter(s => s.name.toLowerCase().includes(query.toLowerCase())).slice(0, 5)
        : [];

    const handleSelectStation = (station: Station) => {
        onChange({
            name: station.name,
            lat: station.lat,
            lon: station.lon,
        });
        setQuery(station.name);
        setIsOpen(false);
    };

    const handleInputChange = (e: React.ChangeEvent<HTMLInputElement>) => {
        setQuery(e.target.value);
        onChange(null);
        setIsOpen(true);
    };

    return (
        <div>
            <label className="text-sm font-medium text-muted-foreground block mb-1.5">
                {label}
            </label>
            <div className="flex gap-2">
                <Popover open={isOpen && filteredStations.length > 0} onOpenChange={setIsOpen}>
                    <PopoverAnchor asChild>
                        <Input
                            ref={inputRef}
                            placeholder={placeholder}
                            value={query}
                            onChange={handleInputChange}
                            onFocus={() => setIsOpen(true)}
                        />
                    </PopoverAnchor>
                    <PopoverContent
                        className="p-0 w-[var(--radix-popover-trigger-width)]"
                        align="start"
                        onOpenAutoFocus={(e) => e.preventDefault()}
                    >
                        <ul className="max-h-48 overflow-y-auto">
                            {filteredStations.map((station) => (
                                <li key={station.id}>
                                    <button
                                        type="button"
                                        className="w-full px-3 py-2 text-left text-sm hover:bg-muted"
                                        onClick={() => handleSelectStation(station)}
                                    >
                                        {station.name}
                                    </button>
                                </li>
                            ))}
                        </ul>
                    </PopoverContent>
                </Popover>
                {onUseCurrentLocation && (
                    <Button
                        variant="outline"
                        size="icon"
                        onClick={onUseCurrentLocation}
                        disabled={isLocating}
                        title="Use current location"
                    >
                        <Crosshair className="h-4 w-4" />
                    </Button>
                )}
                {onPickOnMap && (
                    <Button
                        variant="outline"
                        size="icon"
                        onClick={onPickOnMap}
                        title="Pick on map"
                    >
                        <MapPin className="h-4 w-4" />
                    </Button>
                )}
            </div>
        </div>
    );
}

interface NavigationPanelProps {
    stations: Station[];
}

export function NavigationPanel({ stations }: NavigationPanelProps) {
    const [startLocation, setStartLocation] = useState<Location | null>(null);
    const [endLocation, setEndLocation] = useState<Location | null>(null);
    const [isLocating, setIsLocating] = useState(false);

    const handleUseCurrentLocation = (setLocation: (loc: Location) => void) => {
        if (!navigator.geolocation) {
            alert("Geolocation is not supported by your browser");
            return;
        }

        setIsLocating(true);
        navigator.geolocation.getCurrentPosition(
            (position) => {
                setLocation({
                    name: "Current Location",
                    lat: position.coords.latitude,
                    lon: position.coords.longitude,
                });
                setIsLocating(false);
            },
            (error) => {
                console.error("Error getting location:", error);
                alert("Unable to get your location");
                setIsLocating(false);
            }
        );
    };

    return (
        <div className="p-4">
            <h2 className="font-semibold mb-4">Route Planning</h2>

            <div className="space-y-4">
                <LocationInput
                    label="Start"
                    stations={stations}
                    value={startLocation}
                    onChange={setStartLocation}
                    onUseCurrentLocation={() => handleUseCurrentLocation(setStartLocation)}
                    onPickOnMap={() => {}}
                    isLocating={isLocating}
                />

                <LocationInput
                    label="Destination"
                    stations={stations}
                    value={endLocation}
                    onChange={setEndLocation}
                    onUseCurrentLocation={() => handleUseCurrentLocation(setEndLocation)}
                    onPickOnMap={() => {}}
                    isLocating={isLocating}
                />

                <Button
                    className="w-full"
                    disabled={!startLocation || !endLocation}
                >
                    Find Route
                </Button>

                {(startLocation || endLocation) && (
                    <div className="p-3 bg-muted rounded-lg text-sm space-y-1">
                        {startLocation && (
                            <p><span className="text-muted-foreground">From:</span> {startLocation.name}</p>
                        )}
                        {endLocation && (
                            <p><span className="text-muted-foreground">To:</span> {endLocation.name}</p>
                        )}
                    </div>
                )}
            </div>
        </div>
    );
}
