export interface IAppSettings {
    outputDirs: string[];
    apiUrl: string;
    downloadDir: string;
    wikiSearch: boolean;
    rescanOnStartup: boolean;
    rotatingFetch: boolean;
}

export type TFieldKey = 'apiUrl' | 'downloadDir';
