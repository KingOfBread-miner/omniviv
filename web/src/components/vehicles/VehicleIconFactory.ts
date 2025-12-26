/**
 * Factory for creating vehicle marker icons
 */

const ICON_SIZE = 48;

export const VEHICLE_ICON_SCALE = 0.5;

/**
 * Creates an ImageData icon for a vehicle marker with the line number
 */
export function createVehicleIcon(color: string, lineNumber: string): ImageData {
    const size = ICON_SIZE;
    const canvas = document.createElement("canvas");
    canvas.width = size;
    canvas.height = size;
    const ctx = canvas.getContext("2d")!;

    const center = size / 2;
    const radius = size / 2 - 5;

    // White border
    ctx.beginPath();
    ctx.arc(center, center, radius + 3, 0, Math.PI * 2);
    ctx.fillStyle = "#ffffff";
    ctx.fill();

    // Colored circle
    ctx.beginPath();
    ctx.arc(center, center, radius, 0, Math.PI * 2);
    ctx.fillStyle = color;
    ctx.fill();

    // Line number text
    ctx.fillStyle = "#ffffff";
    ctx.font = `bold ${size * 0.45}px "Open Sans", sans-serif`;
    ctx.textAlign = "center";
    ctx.textBaseline = "middle";
    ctx.fillText(lineNumber, center, center + 1);

    return ctx.getImageData(0, 0, size, size);
}
