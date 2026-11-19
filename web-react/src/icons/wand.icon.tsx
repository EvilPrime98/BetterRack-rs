export function WandIcon({
    size = 14,
    color = '#34c3d1',
}: {
    size?: number;
    color?: string;
} = {}) {
    return (
        <svg
            width={size}
            height={size}
            viewBox="0 0 24 24"
            fill="none"
            xmlns="http://www.w3.org/2000/svg"
        >
            <path d="M4 20L15 9" stroke={color} strokeWidth="2" strokeLinecap="round" />
            <path d="M17 3l0.8 2.2L20 6l-2.2 0.8L17 9l-0.8-2.2L14 6l2.2-0.8L17 3z" fill={color} />
            <path
                d="M6.5 13.5l0.5 1.4 1.4 0.5-1.4 0.5-0.5 1.4-0.5-1.4-1.4-0.5 1.4-0.5 0.5-1.4z"
                fill={color}
            />
        </svg>
    );
}
