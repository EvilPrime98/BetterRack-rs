import { UltraComponent } from 'ultra-light-js';
import { StarIcon } from '../../icons/star.icon';

export function getStarFraction(e: Event): number {
    const rect = (e.currentTarget as Element).getBoundingClientRect();
    const x = (e as MouseEvent).clientX - rect.left;
    return x < rect.width / 2 ? 0.5 : 1;
}

export function StarComponent({
    weight,
    initialFill,
    changeRating,
    onHover,
}: {
    weight: number;
    initialFill: number;
    changeRating: (newRating: number) => void;
    onHover: (previewRating: number) => void;
}) {
    return UltraComponent({
        component: StarIcon({
            size: 18,
            fill: initialFill,
        }),
        eventHandler: {
            mousemove: (e) => onHover(weight - 1 + getStarFraction(e)),
            click: (e) => changeRating(weight - 1 + getStarFraction(e)),
        },
    });
}
