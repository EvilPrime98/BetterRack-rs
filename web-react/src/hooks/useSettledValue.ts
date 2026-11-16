import { useEffect, useState } from 'react';

export function useSettledValue<T>(value: T | null, delay: number): T | null {
    const [settled, setSettled] = useState<T | null>(null);

    if (settled === null && value !== null) setSettled(value);

    useEffect(() => {
        if (value === null || value === settled) return;
        const timer = setTimeout(() => setSettled(value), delay);
        return () => clearTimeout(timer);
    }, [value, settled, delay]);

    return settled;
}
