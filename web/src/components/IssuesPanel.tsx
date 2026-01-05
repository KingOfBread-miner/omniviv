import { useState, useEffect, useMemo } from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";

// Types matching the backend API
type TransportType = "tram" | "bus" | "train" | "unknown";

interface OsmIssue {
    osm_id: number;
    osm_type: string;
    element_type: string;
    issue_type: "missing_ifopt" | "missing_coordinates" | "orphaned_element" | "missing_route_ref" | "missing_name" | "missing_stop_position" | "missing_platform";
    transport_type: TransportType;
    description: string;
    osm_url: string;
    name: string | null;
    lat: number | null;
    lon: number | null;
    detected_at: string;
    suggested_ifopt: string | null;
    suggested_ifopt_name: string | null;
    suggested_ifopt_distance: number | null;
}

interface IssueListResponse {
    issues: OsmIssue[];
    count: number;
}

const API_URL = import.meta.env.VITE_API_URL ?? "http://localhost:3000";

const ISSUE_TYPE_LABELS: Record<OsmIssue["issue_type"], string> = {
    missing_ifopt: "Missing IFOPT",
    missing_coordinates: "Missing Coordinates",
    orphaned_element: "Orphaned Element",
    missing_route_ref: "Missing Route Ref",
    missing_name: "Missing Name",
    missing_stop_position: "Missing Stop Position",
    missing_platform: "Missing Platform",
};

const ISSUE_TYPE_VARIANTS: Record<OsmIssue["issue_type"], "default" | "secondary" | "destructive" | "outline"> = {
    missing_ifopt: "default",
    missing_coordinates: "destructive",
    orphaned_element: "secondary",
    missing_route_ref: "outline",
    missing_name: "secondary",
    missing_stop_position: "outline",
    missing_platform: "outline",
};

const TRANSPORT_TYPE_LABELS: Record<TransportType, string> = {
    tram: "Tram",
    bus: "Bus",
    train: "Train",
    unknown: "Unknown",
};

const TRANSPORT_TYPE_ICONS: Record<TransportType, string> = {
    tram: "🚊",
    bus: "🚌",
    train: "🚆",
    unknown: "❓",
};

interface IssueItemProps {
    issue: OsmIssue;
}

function IssueItem({ issue }: IssueItemProps) {
    const [copied, setCopied] = useState(false);
    const ifoptTag = issue.suggested_ifopt ? `ref:IFOPT=${issue.suggested_ifopt}` : null;

    const handleCopyIfopt = async () => {
        if (ifoptTag) {
            await navigator.clipboard.writeText(ifoptTag);
            setCopied(true);
            setTimeout(() => setCopied(false), 2000);
        }
    };

    return (
        <li className="p-3 hover:bg-muted/50 rounded-lg">
            <div className="flex items-center justify-between gap-2 mb-1">
                <div className="flex items-center gap-2 min-w-0">
                    <Badge variant={ISSUE_TYPE_VARIANTS[issue.issue_type]}>
                        {ISSUE_TYPE_LABELS[issue.issue_type]}
                    </Badge>
                    <span className="text-xs text-muted-foreground">{issue.element_type}</span>
                    <span className="text-xs" title={TRANSPORT_TYPE_LABELS[issue.transport_type]}>
                        {TRANSPORT_TYPE_ICONS[issue.transport_type]}
                    </span>
                </div>
                <Button variant="link" size="sm" asChild className="shrink-0">
                    <a
                        href={issue.osm_url}
                        target="_blank"
                        rel="noopener noreferrer"
                    >
                        Edit
                    </a>
                </Button>
            </div>
            <p className="text-sm font-medium truncate">
                {issue.name || `${issue.osm_type}/${issue.osm_id}`}
            </p>
            <p className="text-xs text-muted-foreground mt-0.5 line-clamp-2">
                {issue.description}
            </p>
            {ifoptTag && (
                <div className="mt-2 p-2 bg-green-50 dark:bg-green-950 rounded border border-green-200 dark:border-green-800">
                    <div className="flex items-center justify-between gap-2">
                        <div className="min-w-0">
                            <p className="text-xs text-green-800 dark:text-green-200 font-medium">Suggested tag:</p>
                            <p className="text-xs text-green-700 dark:text-green-300 font-mono truncate">{ifoptTag}</p>
                            {issue.suggested_ifopt_name && (
                                <p className="text-xs text-green-600 dark:text-green-400 truncate">{issue.suggested_ifopt_name}</p>
                            )}
                            {issue.suggested_ifopt_distance !== null && (
                                <p className="text-xs text-green-500">{issue.suggested_ifopt_distance}m away</p>
                            )}
                        </div>
                        <Button
                            variant="ghost"
                            size="sm"
                            onClick={handleCopyIfopt}
                            className="shrink-0 text-green-700 dark:text-green-300 hover:text-green-900 dark:hover:text-green-100 hover:bg-green-100 dark:hover:bg-green-900"
                        >
                            {copied ? "Copied!" : "Copy"}
                        </Button>
                    </div>
                </div>
            )}
        </li>
    );
}

export function OsmIssuesPanel() {
    const [issues, setIssues] = useState<OsmIssue[]>([]);
    const [isLoading, setIsLoading] = useState(true);
    const [selectedIssueType, setSelectedIssueType] = useState<OsmIssue["issue_type"] | "all">("all");
    const [selectedTransportType, setSelectedTransportType] = useState<TransportType | "all">("all");

    useEffect(() => {
        const fetchIssues = async () => {
            try {
                const response = await fetch(`${API_URL}/api/issues`);
                if (response.ok) {
                    const data: IssueListResponse = await response.json();
                    setIssues(data.issues);
                }
            } catch (error) {
                console.error("Failed to fetch issues:", error);
            } finally {
                setIsLoading(false);
            }
        };

        fetchIssues();
    }, []);

    const filteredIssues = useMemo(() =>
        issues.filter(issue => {
            const matchesIssueType = selectedIssueType === "all" || issue.issue_type === selectedIssueType;
            const matchesTransportType = selectedTransportType === "all" || issue.transport_type === selectedTransportType;
            return matchesIssueType && matchesTransportType;
        }),
        [issues, selectedIssueType, selectedTransportType]
    );

    const issuesByType = useMemo(() =>
        issues.reduce((acc, issue) => {
            acc[issue.issue_type] = (acc[issue.issue_type] || 0) + 1;
            return acc;
        }, {} as Record<string, number>),
        [issues]
    );

    const issuesByTransportType = useMemo(() =>
        issues.reduce((acc, issue) => {
            acc[issue.transport_type] = (acc[issue.transport_type] || 0) + 1;
            return acc;
        }, {} as Record<string, number>),
        [issues]
    );

    return (
        <div className="h-full flex flex-col">
            <div className="p-4 border-b">
                <h2 className="font-semibold">OSM Data Issues ({issues.length})</h2>
            </div>

            {isLoading ? (
                <div className="flex items-center justify-center py-8 flex-1">
                    <p className="text-muted-foreground">Loading issues...</p>
                </div>
            ) : (
                <>
                    <div className="p-4 space-y-3 border-b">
                        {/* Transport type filter */}
                        <div>
                            <span className="text-xs font-medium text-muted-foreground block mb-1.5">Transport Type</span>
                            <div className="flex flex-wrap gap-1">
                                <Button
                                    variant={selectedTransportType === "all" ? "default" : "outline"}
                                    size="sm"
                                    onClick={() => setSelectedTransportType("all")}
                                    className="h-6 text-xs"
                                >
                                    All
                                </Button>
                                {(["tram", "bus", "train"] as TransportType[]).map((type) => {
                                    const count = issuesByTransportType[type] || 0;
                                    if (count === 0) return null;
                                    return (
                                        <Button
                                            key={type}
                                            variant={selectedTransportType === type ? "default" : "outline"}
                                            size="sm"
                                            onClick={() => setSelectedTransportType(type)}
                                            className="h-6 text-xs"
                                        >
                                            {TRANSPORT_TYPE_ICONS[type]} {TRANSPORT_TYPE_LABELS[type]} ({count})
                                        </Button>
                                    );
                                })}
                            </div>
                        </div>

                        {/* Issue type filter */}
                        <div>
                            <span className="text-xs font-medium text-muted-foreground block mb-1.5">Issue Type</span>
                            <div className="flex flex-wrap gap-1">
                                <Button
                                    variant={selectedIssueType === "all" ? "default" : "outline"}
                                    size="sm"
                                    onClick={() => setSelectedIssueType("all")}
                                    className="h-6 text-xs"
                                >
                                    All
                                </Button>
                                {(Object.keys(ISSUE_TYPE_LABELS) as OsmIssue["issue_type"][]).map((type) => {
                                    const count = issuesByType[type] || 0;
                                    if (count === 0) return null;
                                    return (
                                        <Button
                                            key={type}
                                            variant={selectedIssueType === type ? "default" : "outline"}
                                            size="sm"
                                            onClick={() => setSelectedIssueType(type)}
                                            className="h-6 text-xs"
                                        >
                                            {ISSUE_TYPE_LABELS[type]} ({count})
                                        </Button>
                                    );
                                })}
                            </div>
                        </div>
                    </div>

                    <div className="overflow-y-auto flex-1 px-2">
                        {filteredIssues.length === 0 ? (
                            <p className="py-8 text-center text-muted-foreground">No issues in this category</p>
                        ) : (
                            <ul className="divide-y">
                                {filteredIssues.map((issue) => (
                                    <IssueItem key={`${issue.osm_type}-${issue.osm_id}`} issue={issue} />
                                ))}
                            </ul>
                        )}
                    </div>

                    <div className="p-3 border-t text-xs text-muted-foreground">
                        Click "Edit" to fix issues in OpenStreetMap
                    </div>
                </>
            )}
        </div>
    );
}
