import { Terminal } from "lucide-react";
import type { Station, StationPlatform, StationStopPosition } from "../api";
import { getPlatformDisplayName } from "./map/mapUtils";

interface StationPopupProps {
    station: Station;
    onPlatformClick: (platform: StationPlatform | StationStopPosition) => void;
}

export function StationPopup({ station, onPlatformClick }: StationPopupProps) {
    // Get unique platforms by display name (deduplicating within platforms and stop_positions)
    const uniquePlatforms: (StationPlatform | StationStopPosition)[] = [];
    const seenNames = new Set<string>();
    for (const p of station.platforms) {
        const name = getPlatformDisplayName(p);
        if (!seenNames.has(name)) {
            seenNames.add(name);
            uniquePlatforms.push(p);
        }
    }
    for (const sp of station.stop_positions) {
        const name = getPlatformDisplayName(sp);
        if (!seenNames.has(name)) {
            seenNames.add(name);
            uniquePlatforms.push(sp);
        }
    }

    return (
        <div className="p-4 pr-8 bg-popover text-popover-foreground rounded-lg">
            <div className="font-semibold">{station.name || "Unknown station"}</div>
            {uniquePlatforms.length > 0 && (
                <div className="mt-3 border-t border-border pt-2">
                    <div className="text-xs text-muted-foreground mb-1">Platforms ({uniquePlatforms.length})</div>
                    <div className="flex flex-wrap gap-2">
                        {uniquePlatforms.map((p, idx) => (
                            <button
                                key={idx}
                                onClick={() => onPlatformClick(p)}
                                className="px-2 py-1 text-sm font-medium bg-secondary hover:bg-secondary/80 text-secondary-foreground rounded transition-colors"
                            >
                                {getPlatformDisplayName(p)}
                            </button>
                        ))}
                    </div>
                </div>
            )}
            <button
                onClick={() => console.log("Station:", station)}
                className="mt-3 p-1.5 text-muted-foreground hover:text-foreground hover:bg-secondary rounded"
                title="Log to console"
            >
                <Terminal className="w-4 h-4" />
            </button>
        </div>
    );
}
