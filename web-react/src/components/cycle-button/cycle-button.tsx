import type { ButtonHTMLAttributes } from 'react';
import { BRButton } from '@/components/br-button/br-button';
import styles from './cycle-button.module.css';

export function CycleButton<T extends string>({
    state,
    onNext,
    variant = 'secondary',
    className,
    ...props
}: {
    state: T;
    onNext: () => void;
    variant?: 'primary' | 'secondary' | 'ghost';
} & ButtonHTMLAttributes<HTMLButtonElement>) {
    return (
        <BRButton
            text={state}
            variant={variant}
            className={[styles.cycleButton, className].filter(Boolean).join(' ')}
            onClick={onNext}
            {...props}
        />
    );
}
