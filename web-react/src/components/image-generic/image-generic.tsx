import type { ImgHTMLAttributes } from 'react';

export function ImageGen(props: ImgHTMLAttributes<HTMLImageElement>) {
    return <img loading="lazy" decoding="async" {...props} />;
}
