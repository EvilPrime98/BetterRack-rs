import type { MouseEvent } from 'react';

export function getStarFraction(e: MouseEvent): number {
    const rect = e.currentTarget.getBoundingClientRect();
    const x = e.clientX - rect.left;
    return x < rect.width / 2 ? 0.5 : 1;
}
