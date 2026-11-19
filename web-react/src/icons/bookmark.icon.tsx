export function BookmarkIcon({
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
                d="M6 4a2 2 0 0 1 2-2h8a2 2 0 0 1 2 2v17l-6-4.2L6 21V4Z"
                stroke={color}
                strokeWidth="2"
                strokeLinecap="round"
                strokeLinejoin="round"
            />
        </svg>
    );
}
