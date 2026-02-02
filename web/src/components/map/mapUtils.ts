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

// Internationalized time formatter using browser default locale
const timeFormatter = new Intl.DateTimeFormat(undefined, {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
});

// Format time from ISO string using the browser's locale
export function formatTime(isoString: string): string {
    const date = new Date(isoString);
    return timeFormatter.format(date);
}
