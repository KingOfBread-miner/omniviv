import { Activity } from "lucide-react";

export function BackendDiagnosticsPanel() {
    return (
        <div className="p-4 h-full flex flex-col">
            <h2 className="font-semibold mb-4">Backend Diagnostics</h2>
            <div className="flex flex-col items-center justify-center py-12 text-muted-foreground gap-3">
                <Activity className="h-8 w-8" />
                <p className="text-sm text-center">
                    Diagnostics are being migrated to the new GTFS-RT data source.
                </p>
            </div>
        </div>
    );
}
