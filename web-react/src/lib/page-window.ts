export const PRELOAD_WINDOW = 2;

export function getWindowRange(numPages: number, savedPage: number) {
    if (numPages < 1) return null;
    const targetInd = Math.min(Math.max(savedPage, 1), numPages);
    const start = Math.max(1, targetInd - PRELOAD_WINDOW);
    const end = Math.min(numPages, targetInd + PRELOAD_WINDOW);
    return { targetInd, start, end };
}
