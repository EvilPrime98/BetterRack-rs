import { StarIcon } from '@/icons/star.icon';
import { getStarFraction } from './star-component.utils';

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
    return (
        <StarIcon
            size={18}
            fill={initialFill}
            onMouseMove={(e) => onHover(weight - 1 + getStarFraction(e))}
            onClick={(e) => changeRating(weight - 1 + getStarFraction(e))}
        />
    );
}
