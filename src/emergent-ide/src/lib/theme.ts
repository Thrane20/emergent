export const DEFAULT_THEME="charcoal";
export const THEME_STORAGE_KEY="emergent.theme";
export type Theme="green"|"charcoal";

export const DEFAULT_FONT="krypton";
export const FONT_STORAGE_KEY="emergent.font";
export type Font="krypton"|"jetbrains"|"fira";

// Runs before paint; only supported values can reach the root attributes.
export const themeScript=`try{const d=document.documentElement.dataset;const theme=localStorage.getItem(${JSON.stringify(THEME_STORAGE_KEY)});d.theme=theme==="green"||theme==="charcoal"?theme:${JSON.stringify(DEFAULT_THEME)};const font=localStorage.getItem(${JSON.stringify(FONT_STORAGE_KEY)});d.font=font==="jetbrains"||font==="fira"?font:${JSON.stringify(DEFAULT_FONT)}}catch{}`;
