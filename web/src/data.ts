const noCoverSvg = `
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 260 400">
    <defs>
        <linearGradient id="bg" x1="0" y1="0" x2="1" y2="1">
            <stop offset="0%" stop-color="#232222"/>
            <stop offset="100%" stop-color="#181818"/>
        </linearGradient>
    </defs>
    <rect width="260" height="400" fill="url(#bg)"/>
    <g transform="translate(130,158)" stroke="#4a4a4a" stroke-width="3" fill="none" stroke-linejoin="round" stroke-linecap="round">
        <rect x="-42" y="-52" width="84" height="104" rx="8"/>
        <circle cx="-16" cy="-24" r="9"/>
        <path d="M-42 30 L-14 2 L4 20 L26 -10 L42 10"/>
    </g>
    <text x="130" y="248" text-anchor="middle" font-family="Segoe UI, Arial, sans-serif" font-size="16" font-weight="600" letter-spacing="2" fill="#6b6b6b">NO COVER</text>
</svg>
`.trim();

export const NO_IMAGE_URL = `data:image/svg+xml,${encodeURIComponent(noCoverSvg)}`;

export const DEFAULT_IMAGE_SIZE = 120;

export const APP_NAME = 'BetterRack';

export const POLL_INTERVAL_MS = 750;
