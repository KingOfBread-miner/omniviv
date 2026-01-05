import * as React from "react";

interface DateTimePickerProps {
    value: Date;
    onChange: (date: Date) => void;
}

export function DateTimePicker({ value, onChange }: DateTimePickerProps) {
    const dateStr = value.toISOString().split("T")[0];
    const timeStr = value.toTimeString().slice(0, 5);

    const handleDateChange = (e: React.ChangeEvent<HTMLInputElement>) => {
        const newDate = new Date(value);
        const [year, month, day] = e.target.value.split("-").map(Number);
        newDate.setFullYear(year, month - 1, day);
        onChange(newDate);
    };

    const handleTimeChange = (e: React.ChangeEvent<HTMLInputElement>) => {
        const newDate = new Date(value);
        const [hours, minutes] = e.target.value.split(":").map(Number);
        newDate.setHours(hours, minutes);
        onChange(newDate);
    };

    return (
        <div className="flex gap-2">
            <input
                type="date"
                value={dateStr}
                onChange={handleDateChange}
                className="flex h-9 w-full rounded-md border border-input bg-transparent px-3 py-1 text-sm shadow-sm transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
            />
            <input
                type="time"
                value={timeStr}
                onChange={handleTimeChange}
                className="flex h-9 rounded-md border border-input bg-transparent px-3 py-1 text-sm shadow-sm transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
            />
        </div>
    );
}
