import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useTimeSimulation } from "./useTimeSimulation";

describe("useTimeSimulation persistence", () => {
    beforeEach(() => {
        localStorage.clear();
        vi.useFakeTimers();
    });

    afterEach(() => {
        vi.useRealTimers();
    });

    it("starts in real-time mode with no stored state", () => {
        const { result } = renderHook(() => useTimeSimulation());
        expect(result.current.isRealTime).toBe(true);
        expect(result.current.speed).toBe(1);
    });

    it("persists state when setTime is called", () => {
        const { result } = renderHook(() => useTimeSimulation());
        const targetDate = new Date("2025-06-15T10:00:00Z");

        act(() => {
            result.current.setTime(targetDate);
        });

        const stored = JSON.parse(localStorage.getItem("time-simulation")!);
        expect(stored.isRealTime).toBe(false);
        expect(stored.currentTime).toBe(targetDate.toISOString());
    });

    it("persists state when setSpeed is called", () => {
        const { result } = renderHook(() => useTimeSimulation());

        act(() => {
            result.current.setSpeed(5);
        });

        const stored = JSON.parse(localStorage.getItem("time-simulation")!);
        expect(stored.speed).toBe(5);
        expect(stored.isRealTime).toBe(false);
    });

    it("persists state when pause is called", () => {
        const { result } = renderHook(() => useTimeSimulation());

        act(() => {
            result.current.pause();
        });

        const stored = JSON.parse(localStorage.getItem("time-simulation")!);
        expect(stored.speed).toBe(0);
    });

    it("persists state when resetToRealTime is called", () => {
        const { result } = renderHook(() => useTimeSimulation());

        act(() => {
            result.current.setSpeed(10);
        });

        act(() => {
            result.current.resetToRealTime();
        });

        const stored = JSON.parse(localStorage.getItem("time-simulation")!);
        expect(stored.isRealTime).toBe(true);
        expect(stored.speed).toBe(1);
    });

    it("restores simulation mode from localStorage", () => {
        const savedTime = "2025-06-15T10:00:00.000Z";
        localStorage.setItem("time-simulation", JSON.stringify({
            currentTime: savedTime,
            speed: 5,
            isRealTime: false,
        }));

        const { result } = renderHook(() => useTimeSimulation());
        expect(result.current.isRealTime).toBe(false);
        expect(result.current.speed).toBe(5);
        expect(result.current.currentTime.toISOString()).toBe(savedTime);
    });

    it("restores real-time mode with current time from localStorage", () => {
        localStorage.setItem("time-simulation", JSON.stringify({
            currentTime: "2025-06-15T10:00:00.000Z",
            speed: 1,
            isRealTime: true,
        }));

        const { result } = renderHook(() => useTimeSimulation());
        expect(result.current.isRealTime).toBe(true);
        expect(result.current.speed).toBe(1);
        // Should NOT restore the old saved time, should use current wall clock
        expect(result.current.currentTime.toISOString()).not.toBe("2025-06-15T10:00:00.000Z");
    });

    it("handles corrupted localStorage gracefully", () => {
        localStorage.setItem("time-simulation", "not-valid-json");

        const { result } = renderHook(() => useTimeSimulation());
        expect(result.current.isRealTime).toBe(true);
        expect(result.current.speed).toBe(1);
    });

    it("handles invalid date in localStorage gracefully", () => {
        localStorage.setItem("time-simulation", JSON.stringify({
            currentTime: "not-a-date",
            speed: 5,
            isRealTime: false,
        }));

        const { result } = renderHook(() => useTimeSimulation());
        expect(result.current.isRealTime).toBe(true);
        expect(result.current.speed).toBe(1);
    });

    it("handles invalid speed in localStorage gracefully", () => {
        localStorage.setItem("time-simulation", JSON.stringify({
            currentTime: "2025-06-15T10:00:00.000Z",
            speed: "not-a-number",
            isRealTime: false,
        }));

        const { result } = renderHook(() => useTimeSimulation());
        expect(result.current.isRealTime).toBe(true);
        expect(result.current.speed).toBe(1);
    });

    it("handles negative speed in localStorage gracefully", () => {
        localStorage.setItem("time-simulation", JSON.stringify({
            currentTime: "2025-06-15T10:00:00.000Z",
            speed: -5,
            isRealTime: false,
        }));

        const { result } = renderHook(() => useTimeSimulation());
        expect(result.current.isRealTime).toBe(true);
        expect(result.current.speed).toBe(1);
    });
});
