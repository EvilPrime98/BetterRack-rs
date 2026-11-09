import { UltraComponent, type UltraElementProps } from 'ultra-light-js';

interface ImageGenProps extends UltraElementProps {
    attributes?: UltraElementProps['attributes'];
}

export function ImageGen({ attributes, ...props }: ImageGenProps) {
    return UltraComponent({
        component: '<img/>',
        attributes: {
            ...attributes,
            loading: 'lazy',
            decoding: 'async',
        },
        ...props,
    });
}
