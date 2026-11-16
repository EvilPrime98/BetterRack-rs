interface Props {
    size?: number;
}

export function BetterRackIcon({ size = 32 }: Props = {}) {
    return (
        <svg
            width={size}
            height={size}
            viewBox="0 0 32 32"
            fill="none"
            xmlns="http://www.w3.org/2000/svg"
        >
            <rect
                x="1"
                y="1"
                width="30"
                height="30"
                rx="9"
                fill="#111111"
                stroke="var(--accent)"
                strokeWidth="1.5"
            />
            <text
                x="16"
                y="21"
                fontFamily="var(--font-sans)"
                fontWeight="700"
                fontSize="13"
                textAnchor="middle"
                letterSpacing="-0.5"
                fill="var(--accent)"
            >
                BR
            </text>
        </svg>
    );
}
