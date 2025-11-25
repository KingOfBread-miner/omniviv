import { type CSSProperties } from "react";
import Map from "./components/Map";

interface AppProps {
    readonly className?: string;
    readonly style?: CSSProperties;
}

export default function App({ className, style }: AppProps) {
    return (
        <div style={{ ...style }} className={`App h-screen w-screen ${className || ""}`}>
            <Map className="h-full w-full" />
        </div>
    );
}
