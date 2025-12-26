import type { StationPlatform, StationStopPosition } from "../api";

// Extract platform identifier from ref, or from IFOPT ID (last segment after colon)
export function getPlatformDisplayName(platform: StationPlatform | StationStopPosition): string {
    if (platform.ref) return platform.ref;
    if (platform.ref_ifopt) {
        const lastSegment = platform.ref_ifopt.split(":").pop();
        if (lastSegment) return lastSegment.toUpperCase();
    }
    return "?";
}

// Format time from ISO string to HH:MM:SS
export function formatTime(isoString: string): string {
    const date = new Date(isoString);
    return date.toLocaleTimeString("de-DE", { hour: "2-digit", minute: "2-digit", second: "2-digit" });
}
