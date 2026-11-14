const DAY_MS = 86_400_000;
const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];

export function parseDay(raw: string): number | undefined {
    const m = /^(\d{4})-(\d{2})-(\d{2})/.exec(raw);
    if (!m) return undefined;
    const [y, mo, d] = [Number(m[1]), Number(m[2]), Number(m[3])];
    const ms = Date.UTC(y, mo - 1, d);
    const date = new Date(ms);
    if (date.getUTCFullYear() !== y || date.getUTCMonth() !== mo - 1 || date.getUTCDate() !== d)
        return undefined;
    return ms / DAY_MS;
}

export function today(): number {
    return Math.floor(Date.now() / DAY_MS);
}

export function weekStart(day: number): number {
    return day - ((((day + 1) % 7) + 7) % 7);
}

export function dayStart(day: number): string {
    return `${new Date(day * DAY_MS).toISOString().slice(0, 10)}T00:00:00`;
}

function short(day: number): string {
    const date = new Date(day * DAY_MS);
    return `${MONTHS[date.getUTCMonth()]} ${date.getUTCDate()}`;
}

export function rangeLabel(start: number): string {
    return `${short(start)} – ${short(start + 6)}`;
}
