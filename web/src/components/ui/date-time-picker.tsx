import * as React from "react";
import { Input } from "@/components/ui/input";

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
            <Input
                type="date"
                value={dateStr}
                onChange={handleDateChange}
                className="[color-scheme:light] dark:[color-scheme:dark]"
            />
            <Input
                type="time"
                value={timeStr}
                onChange={handleTimeChange}
                className="w-auto [color-scheme:light] dark:[color-scheme:dark]"
            />
        </div>
    );
}
