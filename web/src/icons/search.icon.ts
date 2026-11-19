export function SearchIcon({
    size = 20,
    color = '#fff',
}: {
    size?: number;
    color?: string;
} = {}) {
    return `<svg
        width="${size}"
        height="${size}"
        viewBox="0 0 24 24"
        fill="none"
        xmlns="http://www.w3.org/2000/svg"
    >
        <circle cx="11" cy="11" r="7" stroke="${color}" stroke-width="2"/>
        <line x1="16.65" y1="16.65" x2="21" y2="21" stroke="${color}" stroke-width="2" stroke-linecap="round"/>
    </svg>`;
}
