interface VehiclePopupProps {
    tripId: string;
    lineNumber: string;
    destination: string;
    status: string;
    delayMinutes: number | null;
    currentStopName: string | null;
    nextStopName: string | null;
    routeColors: Map<string, string>;
}

export function VehiclePopup({
    tripId,
    lineNumber,
    destination,
    status,
    delayMinutes,
    currentStopName,
    nextStopName,
    routeColors,
}: VehiclePopupProps) {
    const routeColor = routeColors.get(lineNumber);

    const getStatusText = () => {
        switch (status) {
            case "at_stop":
                return currentStopName ? `At ${currentStopName}` : "At stop";
            case "in_transit":
                return nextStopName ? `En route to ${nextStopName}` : "In transit";
            case "approaching":
                return nextStopName ? `Approaching ${nextStopName}` : "Approaching stop";
            case "completed":
                return "Journey complete";
            default:
                return status;
        }
    };

    const getDelayDisplay = () => {
        // Handle null, undefined, string "null", or 0
        if (delayMinutes == null || delayMinutes === 0) {
            return <span className="text-green-600 font-medium">On time</span>;
        }
        const delay = Number(delayMinutes);
        if (isNaN(delay)) {
            return <span className="text-green-600 font-medium">On time</span>;
        }
        if (delay > 0) {
            return <span className="text-red-600 font-medium">+{delay} min late</span>;
        }
        return <span className="text-blue-600 font-medium">{delay} min early</span>;
    };

    return (
        <div className="p-4 pr-8 min-w-48">
            {/* Header with line number and destination */}
            <div className="flex items-center gap-3">
                <div
                    className="w-10 h-10 rounded-full flex items-center justify-center text-white font-bold text-lg shrink-0"
                    style={{ backgroundColor: routeColor ?? "#3b82f6" }}
                >
                    {lineNumber}
                </div>
                <div>
                    <div className="font-semibold text-gray-900">{destination}</div>
                    <div className="text-sm text-gray-500">Line {lineNumber}</div>
                </div>
            </div>

            {/* Status and delay */}
            <div className="mt-3 border-t pt-2 space-y-1 text-sm">
                <div className="flex justify-between gap-4">
                    <span className="text-gray-600">Status:</span>
                    <span className="text-gray-900 text-right">{getStatusText()}</span>
                </div>
                <div className="flex justify-between gap-4">
                    <span className="text-gray-600">Delay:</span>
                    {getDelayDisplay()}
                </div>
                <div className="flex justify-between gap-4">
                    <span className="text-gray-600">Trip ID:</span>
                    <span className="text-gray-500 font-mono text-xs">{tripId}</span>
                </div>
            </div>
        </div>
    );
}
