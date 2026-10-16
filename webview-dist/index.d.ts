type Props = {
    [key: string]: string | number;
};
/**
 * Optional parameters for the trackError function.
 */
export type TrackErrorOptions = {
    fatal?: boolean;
};
export declare function trackEvent(name: string, props?: Props): Promise<void>;
/**
 * Reports an error to Aptabase: its type, message and stack trace. It appears on the Errors page of the dashboard.
 * Needs the `aptabase:allow-track-error` permission.
 */
export declare function trackError(error: unknown, options?: TrackErrorOptions): Promise<void>;
export {};
