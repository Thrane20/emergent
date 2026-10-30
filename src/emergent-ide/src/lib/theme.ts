export const DEFAULT_THEME="charcoal";
export const THEME_STORAGE_KEY="emergent.theme";
export type Theme="green"|"charcoal";

// Runs before paint; only the two supported values can reach the root attribute.
export const themeScript=`try{const theme=localStorage.getItem(${JSON.stringify(THEME_STORAGE_KEY)});document.documentElement.dataset.theme=theme==="green"||theme==="charcoal"?theme:${JSON.stringify(DEFAULT_THEME)}}catch{}`;
