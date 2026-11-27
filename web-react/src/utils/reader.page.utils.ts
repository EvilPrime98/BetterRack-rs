import { getWindowRange } from '@/lib/page-window';
import { API_URL } from '@/services/library.service';
import { withAuthQuery } from '@/services/server-config.service';

export const RENDER_BEHIND = 3;
export const RENDER_AHEAD = 6;

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
