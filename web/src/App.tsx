import { type CSSProperties } from "react";
import Map from "./components/Map";
import SystemInfo from "./components/SystemInfo";

interface AppProps {
    readonly className?: string;
    readonly style?: CSSProperties;
}

export default function App({ className, style }: AppProps) {
    return (
        <div style={{ ...style }} className={`App h-screen w-screen ${className || ""} relative`}>
            <Map className="h-full w-full" />
            {/* System info overlay in top-left corner */}
            <div className="absolute top-4 left-4 z-10">
                <SystemInfo />
            </div>
        </div>
    );
}
