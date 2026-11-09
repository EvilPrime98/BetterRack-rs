export function MenuIcon({
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
        fill="${color}"
        xmlns="http://www.w3.org/2000/svg"
    >
        <circle cx="12" cy="5" r="2"/>
        <circle cx="12" cy="12" r="2"/>
        <circle cx="12" cy="19" r="2"/>
    </svg>`;
}
