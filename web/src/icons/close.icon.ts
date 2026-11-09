export function CloseIcon({
    size = 16,
    color = '#c7c7c7',
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
        <path d="M6 6l12 12M18 6L6 18" stroke="${color}" stroke-width="2" stroke-linecap="round"/>
    </svg>`;
}
