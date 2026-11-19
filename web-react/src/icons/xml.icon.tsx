export function XmlIcon({
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
            xmlns="http://www.w3.org/2000/svg"
            aria-hidden="true"
            focusable="false"
        >
            <text
                x="12"
                y="16.5"
                textAnchor="middle"
                fontFamily="var(--font-mono, monospace)"
                fontSize="13"
                fontWeight="700"
                fill={color}
            >
                {'</>'}
            </text>
        </svg>
    );
}
