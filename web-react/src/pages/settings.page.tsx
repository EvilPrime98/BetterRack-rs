import { useEffect, useState } from 'react';
import styles from './settings.page.module.css';
import { Layout } from '@/layout';
import { useSettingsStore } from '@/stores/settings.store';
import type { IAppSettings } from '@/settings.types';
import { TextField } from '@/components/text-field/text-field';
import { LibraryFolderRow } from '@/components/library-folder-row/library-folder-row';
import { toast } from '@/services/toast.service';
import { useDocumentTitleStore } from '@/stores/documentTitle.store';
import { BRButton } from '@/components/br-button/br-button';
import { BRCheckbox } from '@/components/br-checkbox/br-checkbox';
import { useLibraryStore } from '@/stores/library.store';
import { useConfirmModalStore } from '@/stores/confirmModal.store';

type SettingsDraft = Pick<
    IAppSettings,
    'apiUrl' | 'downloadDir' | 'wikiSearch' | 'rescanOnStartup' | 'rotatingFetch'
>;
type SetDraft = React.Dispatch<React.SetStateAction<SettingsDraft>>;

const WIKI_SEARCH_TIP =
    'Comics are always identified from ComicInfo.xml. When enabled, comics without it are looked up on the wiki.';

const ROTATING_FETCH_TIP =
    'Send every download through the rotating fetch (changing identity per attempt). When off, only PixelDrain hosts use it.';

function DownloadsSection({ draft, setDraft }: { draft: SettingsDraft; setDraft: SetDraft }) {
    return (
        <section className={styles.section}>
            <div className={styles.fieldGroup}>
                <h2 className={styles.sectionTitle}>Downloads</h2>

                <div className={styles.field}>
                    <label className={styles.label} htmlFor="settings-downloadDir">
                        Download folder
                    </label>

                    <div className={styles.addRow}>
                        <input
                            id="settings-downloadDir"
                            type="text"
                            className={styles.input}
                            placeholder="/path/to/downloads"
                            value={draft.downloadDir ?? ''}
                            onChange={(e) =>
                                setDraft((d) => ({ ...d, downloadDir: e.target.value }))
                            }
                        />
                    </div>
                </div>

                <div className={styles.tipWrap} title={ROTATING_FETCH_TIP}>
                    <BRCheckbox
                        text="Rotate identity"
                        checked={draft.rotatingFetch}
                        onChange={(e) => {
                            const checked = e.currentTarget.checked;
                            setDraft((d) => ({ ...d, rotatingFetch: checked }));
                        }}
                    />
                </div>
            </div>
        </section>
    );
}

function StoreSection({ setDraft }: { setDraft: SetDraft }) {
    return (
        <section className={styles.section}>
            <div className={styles.fieldGroup}>
                <h2 className={styles.sectionTitle}>Store configuration</h2>

                <TextField
                    fieldKey="apiUrl"
                    label="API URL"
                    placeholder="https://example.com/wp-json/wp/v2"
                    onChange={(e) =>
                        setDraft((d) => ({ ...d, apiUrl: (e.target as HTMLInputElement).value }))
                    }
                />
            </div>
        </section>
    );
}

function LibraryFoldersSection({ outputDirs }: { outputDirs: string[] }) {
    const [folderError, setFolderError] = useState('');
    const [folderPath, setFolderPath] = useState('');

    function clearFolderError() {
        setFolderError('');
    }

    async function addFolder(rawValue: string) {
        const value = rawValue.trim();
        if (!value) return;
        clearFolderError();
        try {
            await useSettingsStore.getState().addLibraryFolder(value);
            setFolderPath('');
            toast.success('Folder added');
            useLibraryStore.getState().refreshLibrary({ silent: true });
        } catch (e) {
            const message =
                e instanceof Error ? e.message : 'There was an error adding the folder.';
            setFolderError(message);
            toast.error(message);
        }
    }

    function onAddFolder() {
        addFolder(folderPath);
    }

    return (
        <section className={`${styles.section}`}>
            <h2 className={styles.sectionTitle}>Library folders</h2>

            <ul className={styles.folderList}>
                {outputDirs.length === 0 ? (
                    <li className={styles.empty}>No library folders configured yet.</li>
                ) : (
                    outputDirs.map((dir) => (
                        <LibraryFolderRow
                            key={dir}
                            dir={dir}
                            clearFolderError={clearFolderError}
                            setFolderError={setFolderError}
                        />
                    ))
                )}
            </ul>

            <div className={styles.addRow}>
                <input
                    type="text"
                    className={styles.input}
                    placeholder="Folder path"
                    aria-label="Folder path"
                    value={folderPath}
                    onChange={(e) => setFolderPath(e.target.value)}
                    onKeyDown={(e) => {
                        if (e.key === 'Enter') onAddFolder();
                    }}
                />

                <BRButton text="Add Folder" onClick={onAddFolder} />
            </div>

            <p className={styles.errorText}>{folderError}</p>
        </section>
    );
}

function IdentificationSection({ draft, setDraft }: { draft: SettingsDraft; setDraft: SetDraft }) {
    async function onReidentifyAll() {
        const confirmed = await useConfirmModalStore.getState().confirmDialog({
            title: 'Re-identify all comics?',
            message:
                'This will re-run identification for every comic in your library, overwriting any existing matches.',
            confirmLabel: 'Re-identify',
        });
        if (!confirmed) return;
        useLibraryStore.getState().reidentifyAll();
    }

    return (
        <section className={styles.section}>
            <div className={styles.fieldGroup}>
                <h2 className={styles.sectionTitle}>Identification</h2>

                <div className={styles.fieldGroup}>
                    <div className={styles.tipWrap} title={WIKI_SEARCH_TIP}>
                        <BRCheckbox
                            text="Search the wiki for metadata"
                            checked={draft.wikiSearch}
                            onChange={(e) => {
                                const checked = e.currentTarget.checked;
                                setDraft((d) => ({ ...d, wikiSearch: checked }));
                            }}
                        />
                    </div>

                    <div className={styles.buttonRow}>
                        <BRButton
                            variant="secondary"
                            text="Re-identify all"
                            onClick={onReidentifyAll}
                        />
                    </div>
                </div>

                <BRCheckbox
                    text="Re-scan on start up"
                    checked={draft.rescanOnStartup}
                    onChange={(e) => {
                        const checked = e.currentTarget.checked;
                        setDraft((d) => ({ ...d, rescanOnStartup: checked }));
                    }}
                />
            </div>
        </section>
    );
}

export function SettingsPage() {
    const settings = useSettingsStore((s) => s.settings);
    const setTitle = useDocumentTitleStore((s) => s.setTitle);
    const [settingsError, setSettingsError] = useState('');
    const [draft, setDraft] = useState<SettingsDraft>({
        apiUrl: settings.apiUrl,
        downloadDir: settings.downloadDir,
        wikiSearch: settings.wikiSearch,
        rescanOnStartup: settings.rescanOnStartup,
        rotatingFetch: settings.rotatingFetch,
    });

    async function onSave() {
        const partial: Partial<Omit<IAppSettings, 'outputDirs'>> = { ...draft };

        setSettingsError('');

        try {
            await useSettingsStore.getState().updateSettings(partial);
            toast.success('Settings saved');
        } catch (e) {
            const message = e instanceof Error ? e.message : 'Something went wrong.';
            setSettingsError(message);
            toast.error(message);
        }
    }

    useEffect(() => {
        setTitle('Settings');
    }, [setTitle]);

    useEffect(() => {
        setDraft({
            apiUrl: settings.apiUrl,
            downloadDir: settings.downloadDir,
            wikiSearch: settings.wikiSearch,
            rescanOnStartup: settings.rescanOnStartup,
            rotatingFetch: settings.rotatingFetch,
        });
    }, [settings]);

    return (
        <Layout>
            <section className={styles.page}>
                <header className={styles.header}>
                    <h1 className={styles.title}>Settings</h1>

                    <div className={styles.headerActions}>
                        <p className={styles.errorText}>{settingsError}</p>

                        <BRButton text="Save" onClick={onSave} />
                    </div>
                </header>

                <div className={styles.grid}>
                    <div className={styles.column}>
                        <DownloadsSection draft={draft} setDraft={setDraft} />
                        <StoreSection setDraft={setDraft} />
                    </div>

                    <div className={styles.column}>
                        <LibraryFoldersSection outputDirs={settings.outputDirs} />
                        <IdentificationSection draft={draft} setDraft={setDraft} />
                    </div>
                </div>
            </section>
        </Layout>
    );
}
