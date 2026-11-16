export function PinIcon({
    size = 16,
    color = '#c7c7c7',
    filled = false,
    rotate = 0,
}: {
    size?: number;
    color?: string;
    filled?: boolean;
    rotate?: number;
} = {}) {
    return (
        <svg
            style={{ transform: `rotate(${rotate}deg)` }}
            width={size}
            height={size}
            viewBox="0 0 24 24"
            fill="none"
            xmlns="http://www.w3.org/2000/svg"
        >
            <path
                d="M9 3h6l-1 6 3 3v2H7v-2l3-3-1-6zM12 14v7"
                stroke={color}
                strokeWidth="2"
                strokeLinecap="round"
                strokeLinejoin="round"
                fill={filled ? color : 'none'}
            />
        </svg>
    );
}
