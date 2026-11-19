import { useId, type SVGProps } from 'react';

export function StarIcon({
    size = 24,
    fill = 1,
    ...props
}: {
    size?: number;
    fill?: number;
} & Omit<SVGProps<SVGSVGElement>, 'width' | 'height' | 'viewBox' | 'fill'>) {
    const percent = Math.max(0, Math.min(1, fill));
    const clipWidth = 16 * percent;
    const clipId = `star-fill-${useId()}`;

    return (
        <svg
            width={size}
            height={size}
            viewBox="0 0 16 16"
            xmlns="http://www.w3.org/2000/svg"
            {...props}
        >
            <defs>
                <clipPath id={clipId}>
                    <rect
                        x="0"
                        y="0"
                        width={clipWidth}
                        height="16"
                        className="fillRect"
                        style={{ transition: 'width 160ms ease' }}
                    />
                </clipPath>
            </defs>

            <path
                d="M9 0H7L5.51292 4.57681H0.700554L0.0825195 6.47893L3.97581 9.30756L2.48873 13.8843L4.10677 15.0599L8 12.2313L11.8933 15.0599L13.5113 13.8843L12.0242 9.30754L15.9175 6.47892L15.2994 4.57681H10.4871L9 0Z"
                fill="#717479"
            />
            <path
                clipPath={`url(#${clipId})`}
                d="M9 0H7L5.51292 4.57681H0.700554L0.0825195 6.47893L3.97581 9.30756L2.48873 13.8843L4.10677 15.0599L8 12.2313L11.8933 15.0599L13.5113 13.8843L12.0242 9.30754L15.9175 6.47892L15.2994 4.57681H10.4871L9 0Z"
                fill="#fefcf3"
            />
        </svg>
    );
}
