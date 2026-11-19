import type { IUltraCompStateStateful, UltraElementProps } from 'ultra-light-js';
import { BRButton } from '@/components/br-button/br-button';
import styles from './cycle-button.module.css';

export function CycleButton<T extends string>({
    state,
    onNext,
    variant = 'secondary',
    ...props
}: {
    state: IUltraCompStateStateful<T>;
    onNext: () => void;
    variant?: 'primary' | 'secondary' | 'ghost';
} & UltraElementProps) {
    const onStateChange = ($button: HTMLElement) => {
        $button.textContent = state.get();
    };

    return BRButton({
        ...props,

        text: state.get(),

        className: [...(props.className ? props.className : []), styles.cycleButton],

        variant,

        onMount: [onStateChange, ...(props.onMount ?? [])],

        trigger: [
            { subscriber: state.subscribe, triggerFunction: onStateChange },
            ...(props.trigger ?? []),
        ],

        eventHandler: {
            click: onNext,
            ...props.eventHandler,
        },
    });
}
