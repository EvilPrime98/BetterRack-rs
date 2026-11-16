export const SCROLL_STEP_PX = 80;

export type TReaderMove =
    { type: 'by'; unit: 'viewport' | 'px'; amount: number } | { type: 'to'; edge: 'start' | 'end' };

export function readerMoveFor(key: string, shift: boolean): TReaderMove | null {
    switch (key) {
        case 'PageDown':
            return { type: 'by', unit: 'viewport', amount: 1 };
        case 'PageUp':
            return { type: 'by', unit: 'viewport', amount: -1 };
        case ' ':
            return { type: 'by', unit: 'viewport', amount: shift ? -1 : 1 };
        case 'ArrowDown':
            return { type: 'by', unit: 'px', amount: SCROLL_STEP_PX };
        case 'ArrowUp':
            return { type: 'by', unit: 'px', amount: -SCROLL_STEP_PX };
        case 'Home':
            return { type: 'to', edge: 'start' };
        case 'End':
            return { type: 'to', edge: 'end' };
        default:
            return null;
    }
}

export function applyMove(
    move: TReaderMove,
    { offset, viewport, max }: { offset: number; viewport: number; max: number },
): number {
    let next: number;
    if (move.type === 'to') next = move.edge === 'start' ? 0 : max;
    else next = offset + (move.unit === 'viewport' ? viewport * move.amount : move.amount);
    return Math.min(max, Math.max(0, next));
}
