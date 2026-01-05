import { useCallback, useEffect, useRef, useState } from "react";

export interface TimeSimulationState {
    /** The simulated current time */
    currentTime: Date;
    /** Speed multiplier (1 = real-time, 0 = paused) */
    speed: number;
    /** Whether simulation is using real time or custom time */
    isRealTime: boolean;
}

export interface TimeSimulationControls {
    /** Set a specific date/time for the simulation */
    setTime: (date: Date) => void;
    /** Set the simulation speed (0 = paused, 1 = real-time, 2 = 2x, etc.) */
    setSpeed: (speed: number) => void;
    /** Reset to real-time */
    resetToRealTime: () => void;
    /** Pause the simulation */
    pause: () => void;
    /** Resume the simulation at current speed (or 1x if was paused) */
    resume: () => void;
}

export type UseTimeSimulationResult = TimeSimulationState & TimeSimulationControls;

const UPDATE_INTERVAL = 50; // Update every 50ms for smooth animation

export function useTimeSimulation(): UseTimeSimulationResult {
    const [simulatedTime, setSimulatedTime] = useState<Date>(new Date());
    const [speed, setSpeedState] = useState(1);
    const [isRealTime, setIsRealTime] = useState(true);

    // Track the last update time for calculating elapsed time
    const lastUpdateRef = useRef<number>(Date.now());
    const speedRef = useRef(speed);
    const isRealTimeRef = useRef(isRealTime);

    // Keep refs in sync with state
    useEffect(() => {
        speedRef.current = speed;
    }, [speed]);

    useEffect(() => {
        isRealTimeRef.current = isRealTime;
    }, [isRealTime]);

    // Update the simulated time based on speed
    useEffect(() => {
        const interval = setInterval(() => {
            const now = Date.now();
            const elapsed = now - lastUpdateRef.current;
            lastUpdateRef.current = now;

            if (isRealTimeRef.current) {
                // In real-time mode, just use current time
                setSimulatedTime(new Date());
            } else if (speedRef.current > 0) {
                // In simulation mode, advance time by elapsed * speed
                setSimulatedTime(prev => new Date(prev.getTime() + elapsed * speedRef.current));
            }
            // If speed is 0 (paused), don't update the time
        }, UPDATE_INTERVAL);

        return () => clearInterval(interval);
    }, []);

    const setTime = useCallback((date: Date) => {
        setSimulatedTime(date);
        setIsRealTime(false);
        lastUpdateRef.current = Date.now();
    }, []);

    const setSpeed = useCallback((newSpeed: number) => {
        setSpeedState(newSpeed);
        // Changing speed enters simulation mode (except when setting to 1x in real-time mode)
        if (newSpeed !== 1 || !isRealTimeRef.current) {
            setIsRealTime(false);
        }
        lastUpdateRef.current = Date.now();
    }, []);

    const resetToRealTime = useCallback(() => {
        setIsRealTime(true);
        setSpeedState(1);
        setSimulatedTime(new Date());
        lastUpdateRef.current = Date.now();
    }, []);

    const pause = useCallback(() => {
        setSpeedState(0);
    }, []);

    const resume = useCallback(() => {
        setSpeedState(prev => prev === 0 ? 1 : prev);
        lastUpdateRef.current = Date.now();
    }, []);

    return {
        currentTime: simulatedTime,
        speed,
        isRealTime,
        setTime,
        setSpeed,
        resetToRealTime,
        pause,
        resume,
    };
}
