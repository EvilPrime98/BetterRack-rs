import { UltraComponent } from 'ultra-light-js';
import styles from './pull-list.module.css';
import { PULL_LIST_CTX } from '@/context/pull-list.context';
import { STORE_PAGE_CONTEXT } from '@/context/store-page.context';
import { newCount } from '@/lib/pull-list';
import { BRButton } from '@/components/br-button/br-button';

export function PullList() {
    let $keyword!: HTMLInputElement;
    let $exclude!: HTMLInputElement;

    async function submit() {
        const added = await PULL_LIST_CTX.add($keyword.value, $exclude.value);
        if (!added) return;
        $keyword.value = '';
        $exclude.value = '';
    }

    function onKeydown(e: Event) {
        if ((e as KeyboardEvent).key === 'Enter') submit();
    }

    function onPrefill() {
        const keyword = PULL_LIST_CTX.prefill.get();
        if (!keyword || !$keyword) return;
        $keyword.value = keyword;
        $exclude.value = '';
        $keyword.focus();
        PULL_LIST_CTX.prefill.set('');
    }

    function chip(id: string, label: string, fresh: number, active: boolean) {
        const $chip = document.createElement('div');
        $chip.className = `${styles.chip} ${active ? styles.active : ''} ${fresh > 0 ? styles.fresh : ''}`;

        const $name = document.createElement('button');
        $name.type = 'button';
        $name.textContent = fresh > 0 ? `${label} · ${fresh} new` : label;
        $name.addEventListener('click', () =>
            PULL_LIST_CTX.focus.set(PULL_LIST_CTX.focus.get() === id ? '' : id),
        );
        $chip.appendChild($name);

        if (active && fresh > 0) {
            const $seen = document.createElement('button');
            $seen.type = 'button';
            $seen.textContent = 'Mark seen';
            $seen.addEventListener('click', () => PULL_LIST_CTX.markSeen(id));
            $chip.appendChild($seen);
        }

        const $remove = document.createElement('button');
        $remove.type = 'button';
        $remove.textContent = '×';
        $remove.setAttribute('aria-label', `Unfollow ${label}`);
        $remove.addEventListener('click', () => PULL_LIST_CTX.remove(id));
        $chip.appendChild($remove);
        return $chip;
    }

    function renderChips($div: HTMLElement) {
        const results = STORE_PAGE_CONTEXT.results.get();
        const focus = PULL_LIST_CTX.focus.get();
        $div.replaceChildren(
            ...PULL_LIST_CTX.items
                .get()
                .map((e) => chip(e.id, e.keyword, newCount(e, results), e.id === focus)),
        );
    }

    return UltraComponent({
        component: '<div></div>',
        className: [styles.pullList],
        children: [
            UltraComponent({
                component: '<div></div>',
                className: [styles.form],
                children: [
                    `<h2 class="${styles.heading}">Pull list</h2>`,
                    UltraComponent({
                        component: '<input type="text" />',
                        className: [styles.input],
                        attributes: {
                            placeholder: 'Follow a series (title keyword)…',
                            'aria-label': 'Pull-list keyword',
                        },
                        onMount: [($el: HTMLElement) => void ($keyword = $el as HTMLInputElement)],
                        eventHandler: { keydown: onKeydown },
                        trigger: [
                            {
                                subscriber: PULL_LIST_CTX.prefill.subscribe,
                                triggerFunction: onPrefill,
                            },
                        ],
                    }),
                    UltraComponent({
                        component: '<input type="text" />',
                        className: [styles.input],
                        attributes: {
                            placeholder: 'Exclude (comma separated)',
                            'aria-label': 'Pull-list exclude terms',
                        },
                        onMount: [($el: HTMLElement) => void ($exclude = $el as HTMLInputElement)],
                        eventHandler: { keydown: onKeydown },
                    }),
                    BRButton({
                        text: 'Add',
                        variant: 'secondary',
                        eventHandler: { click: submit },
                    }),
                ],
            }),
            UltraComponent({
                component: '<div></div>',
                className: [styles.chips],
                onMount: [renderChips],
                trigger: [
                    {
                        subscriber: [
                            PULL_LIST_CTX.items.subscribe,
                            PULL_LIST_CTX.focus.subscribe,
                            STORE_PAGE_CONTEXT.results.subscribe,
                        ],
                        triggerFunction: renderChips,
                    },
                ],
            }),
        ],
    });
}
