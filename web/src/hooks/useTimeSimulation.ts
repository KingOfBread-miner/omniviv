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
const STORAGE_KEY = "time-simulation";

interface PersistedTimeSimulation {
    currentTime: string; // ISO 8601
    speed: number;
    isRealTime: boolean;
}

function loadTimeState(): { time: Date; speed: number; isRealTime: boolean } {
    try {
        const stored = localStorage.getItem(STORAGE_KEY);
        if (stored) {
            const parsed: PersistedTimeSimulation = JSON.parse(stored);

            if (typeof parsed.speed !== "number" || parsed.speed < 0 || !isFinite(parsed.speed)) {
                throw new Error("Invalid speed value");
            }
            if (typeof parsed.isRealTime !== "boolean") {
                throw new Error("Invalid isRealTime value");
            }

            if (parsed.isRealTime) {
                return { time: new Date(), speed: 1, isRealTime: true };
            }

            const restoredTime = new Date(parsed.currentTime);
            if (isNaN(restoredTime.getTime())) {
                throw new Error("Invalid stored time");
            }

            return {
                time: restoredTime,
                speed: parsed.speed,
                isRealTime: false,
            };
        }
    } catch (e) {
        console.error("Failed to load time simulation state from localStorage:", e);
    }
    return { time: new Date(), speed: 1, isRealTime: true };
}

function saveTimeState(state: PersistedTimeSimulation): void {
    try {
        localStorage.setItem(STORAGE_KEY, JSON.stringify(state));
    } catch (e) {
        console.error("Failed to save time simulation state to localStorage:", e);
    }
}

export function useTimeSimulation(): UseTimeSimulationResult {
    const [initialState] = useState(loadTimeState);
    const [simulatedTime, setSimulatedTime] = useState<Date>(initialState.time);
    const [speed, setSpeedState] = useState(initialState.speed);
    const [isRealTime, setIsRealTime] = useState(initialState.isRealTime);

    // Track the last update time for calculating elapsed time
    const lastUpdateRef = useRef<number>(Date.now());
    const speedRef = useRef(speed);
    const isRealTimeRef = useRef(isRealTime);
    const simulatedTimeRef = useRef(simulatedTime);

    // Keep refs in sync with state
    useEffect(() => {
        speedRef.current = speed;
    }, [speed]);

    useEffect(() => {
        isRealTimeRef.current = isRealTime;
    }, [isRealTime]);

    useEffect(() => {
        simulatedTimeRef.current = simulatedTime;
    }, [simulatedTime]);

    // Save on beforeunload to capture final state before tab close
    useEffect(() => {
        const handleBeforeUnload = () => {
            saveTimeState({
                currentTime: simulatedTimeRef.current.toISOString(),
                speed: speedRef.current,
                isRealTime: isRealTimeRef.current,
            });
        };
        window.addEventListener("beforeunload", handleBeforeUnload);
        return () => window.removeEventListener("beforeunload", handleBeforeUnload);
    }, []);

    // Helper to persist current state (called on user actions, not on every tick)
    const persistState = useCallback((time: Date, spd: number, realTime: boolean) => {
        saveTimeState({
            currentTime: time.toISOString(),
            speed: spd,
            isRealTime: realTime,
        });
    }, []);

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
        persistState(date, speedRef.current, false);
    }, [persistState]);

    const setSpeed = useCallback((newSpeed: number) => {
        setSpeedState(newSpeed);
        if (newSpeed !== 1 || !isRealTimeRef.current) {
            setIsRealTime(false);
            persistState(simulatedTimeRef.current, newSpeed, false);
        } else {
            persistState(simulatedTimeRef.current, newSpeed, isRealTimeRef.current);
        }
        lastUpdateRef.current = Date.now();
    }, [persistState]);

    const resetToRealTime = useCallback(() => {
        const now = new Date();
        setIsRealTime(true);
        setSpeedState(1);
        setSimulatedTime(now);
        lastUpdateRef.current = Date.now();
        persistState(now, 1, true);
    }, [persistState]);

    const pause = useCallback(() => {
        setSpeedState(0);
        persistState(simulatedTimeRef.current, 0, isRealTimeRef.current);
    }, [persistState]);

    const resume = useCallback(() => {
        setSpeedState(prev => {
            const newSpeed = prev === 0 ? 1 : prev;
            persistState(simulatedTimeRef.current, newSpeed, isRealTimeRef.current);
            return newSpeed;
        });
        lastUpdateRef.current = Date.now();
    }, [persistState]);

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
