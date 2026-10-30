import type { Metadata } from "next";
import localFont from "next/font/local";
import {DEFAULT_THEME,themeScript} from "@/lib/theme";
import {EditorFrame} from "@/components/editor-navigation";
import "./globals.css";
import "./object-creator.css";
import "./scene-viewer.css";
import "./statements.css";
import "./scene-placement.css";
import "./inspection.css";
import "./scene-graph.css";
import "./noise-filter.css";
import "./material.css";
import "./terrain.css";
import "./tween.css";
import "./brain.css";
import "./structure.css";
import "./particles.css";
import "./themes.css";
import "./editor-navigation.css";

const krypton = localFont({
  src: "./fonts/MonaspaceKryptonVar.woff2",
  variable: "--font-krypton",
  weight: "200 800",
  display: "swap",
});

export const metadata: Metadata = {
  title: "Emergent IDE",
  description: "Reference-oriented project mode for Emergent game projects.",
};

export default function RootLayout({ children }: LayoutProps<"/">) {
  return (
    <html
      lang="en"
      data-theme={DEFAULT_THEME}
      suppressHydrationWarning
      className={`${krypton.variable} h-full antialiased`}
    >
      <head><script dangerouslySetInnerHTML={{__html:themeScript}}/></head>
      <body className="min-h-full flex flex-col"><EditorFrame>{children}</EditorFrame></body>
    </html>
  );
}
