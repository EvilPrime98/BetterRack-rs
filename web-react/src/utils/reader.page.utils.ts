import { API_URL } from '@/services/library.service';
import { withAuthQuery } from '@/services/server-config.service';

export const PRELOAD_WINDOW = 2;
export const RENDER_BEHIND = 3;
export const RENDER_AHEAD = 6;

export function getWindowRange(numPages: number, savedPage: number) {
    if (numPages < 2) return null;
    const targetInd = Math.min(Math.max(savedPage - 1, 1), numPages - 1);
    const start = Math.max(1, targetInd - PRELOAD_WINDOW);
    const end = Math.min(numPages - 1, targetInd + PRELOAD_WINDOW);
    return { targetInd, start, end };
}

export async function preloadWindow(uid: string, numPages: number, savedPage: number) {
    const range = getWindowRange(numPages, savedPage);
    if (!range) return;
    const loads: Promise<void>[] = [];
    for (let ind = range.start; ind <= range.end; ++ind) {
        loads.push(
            new Promise<void>((resolve) => {
                const img = new Image();
                img.onload = img.onerror = () => resolve();
                img.src = withAuthQuery(`${API_URL}/read/${uid}/pages/${ind}`);
            }),
        );
    }
    await Promise.all(loads);
}
