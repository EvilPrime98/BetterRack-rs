import { UltraComponent, type UltraElementProps } from 'ultra-light-js';
import type { TFieldKey } from '@/settings.types';
import styles from '@/pages/settings.page.module.css';
import { SETTINGS_CONTEXT } from '@/context/settings.context';

export function TextField({
    key,
    label,
    placeholder,
    ...props
}: {
    key: TFieldKey;
    label: string;
    placeholder?: string;
} & UltraElementProps) {
    function onFieldMount($el: HTMLElement) {
        const $input = $el as HTMLInputElement;
        $input.value = SETTINGS_CONTEXT.settings.get()[key] ?? '';
    }

    return UltraComponent({
        component: '<div></div>',

        className: [styles.field],

        children: [
            `<label class="${styles.label}" for="settings-${key}">${label}</label>`,

            UltraComponent({
                ...props,

                component: `<input id="settings-${key}" type="text" />`,

                className: [...(props.className ? props.className : []), styles.input],

                attributes: {
                    ...props.attributes,
                    placeholder: placeholder ?? '',
                },

                onMount: [...(props.onMount ? props.onMount : []), onFieldMount],

                trigger: [
                    ...(props.trigger ? props.trigger : []),
                    {
                        subscriber: SETTINGS_CONTEXT.settings.subscribe,
                        triggerFunction: onFieldMount,
                        defer: true,
                    },
                ],
            }),
        ],
    });
}
