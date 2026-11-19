export function TrashIcon({
    size = 16,
    color = 'currentColor',
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
            <path
                d="M4 7h16M9 7V4a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v3m3 0-.87 13.14A2 2 0 0 1 15.14 22H8.86a2 2 0 0 1-1.99-1.86L6 7"
                stroke={color}
                strokeWidth="2"
                strokeLinecap="round"
                strokeLinejoin="round"
            />
            <path d="M10 11v6M14 11v6" stroke={color} strokeWidth="2" strokeLinecap="round" />
        </svg>
    );
}
