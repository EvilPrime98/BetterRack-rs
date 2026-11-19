export function ChevronDownIcon({
    size = 14,
    color = '#e8a33d',
    orientation = 'down',
}: {
    size?: number;
    color?: string;
    orientation?: 'up' | 'down' | 'left' | 'right';
} = {}) {
    const rotations = {
        down: 0,
        up: 180,
        left: 90,
        right: -90,
    };

    return `<svg
        width="${size}"
        height="${size}"
        viewBox="0 0 24 24"
        fill="none"
        xmlns="http://www.w3.org/2000/svg"
        style="transform: rotate(${rotations[orientation]}deg)"
    >
        <path d="M6 9l6 6 6-6" stroke="${color}" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"/>
    </svg>`;
}
