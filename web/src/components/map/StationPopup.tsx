import React from "react";
import { Platform } from "./types";

interface StationPopupProps {
    stationName: string;
    platforms: Platform[];
    onPlatformClick: (platform: Platform) => void;
    onClose: () => void;
}

export class StationPopup extends React.Component<StationPopupProps> {
    render() {
        const { stationName, platforms, onPlatformClick, onClose } = this.props;

        return (
            <div className="bg-white rounded-lg shadow-lg border border-gray-200">
                <div className="flex items-start justify-between p-4 pb-3 border-b border-gray-100">
                    <h3 className="font-bold text-lg">{stationName}</h3>
                    <button
                        onClick={onClose}
                        className="ml-4 text-gray-400 hover:text-gray-600 transition-colors"
                        aria-label="Close"
                    >
                        <svg
                            width="20"
                            height="20"
                            viewBox="0 0 20 20"
                            fill="none"
                            stroke="currentColor"
                            strokeWidth="2"
                            strokeLinecap="round"
                        >
                            <line x1="5" y1="5" x2="15" y2="15" />
                            <line x1="15" y1="5" x2="5" y2="15" />
                        </svg>
                    </button>
                </div>
                <div className="p-4 pt-3">
                    <p className="text-sm font-semibold mb-2">Platforms ({platforms.length}):</p>
                    <ul className="list-disc list-inside space-y-1">
                        {platforms.map((platform, idx) => (
                            <li
                                key={idx}
                                className="text-sm cursor-pointer hover:text-blue-600 hover:underline"
                                onClick={() => onPlatformClick(platform)}
                            >
                                {platform.name}
                            </li>
                        ))}
                    </ul>
                </div>
            </div>
        );
    }
}
