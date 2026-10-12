import { webDarkTheme, webLightTheme, type Theme } from "@fluentui/react-components";

const LIGHT = "#176B73";
const LIGHT_HOVER = "#11565D";
const LIGHT_PRESSED = "#0C4147";
const DARK = "#68C6C4";
const DARK_HOVER = "#8AD8D3";
const DARK_PRESSED = "#43A8A8";

function paint(base: Theme, bg: string, hover: string, pressed: string, fg: string, onBrand: string): Theme {
  return {
    ...base,
    colorBrandBackground: bg,
    colorBrandBackgroundHover: hover,
    colorBrandBackgroundPressed: pressed,
    colorBrandBackgroundSelected: hover,
    colorBrandBackground2: bg,
    colorBrandBackground2Hover: hover,
    colorBrandBackground2Pressed: pressed,
    colorCompoundBrandBackground: bg,
    colorCompoundBrandBackgroundHover: hover,
    colorCompoundBrandBackgroundPressed: pressed,
    colorBrandForeground1: fg,
    colorBrandForeground2: fg,
    colorBrandForegroundLink: fg,
    colorBrandForegroundLinkHover: hover,
    colorBrandForegroundLinkPressed: pressed,
    colorCompoundBrandForeground1: fg,
    colorCompoundBrandForeground1Hover: hover,
    colorCompoundBrandForeground1Pressed: pressed,
    colorBrandForegroundOnLight: LIGHT,
    colorBrandForegroundOnLightHover: LIGHT_HOVER,
    colorBrandForegroundOnLightPressed: LIGHT_PRESSED,
    colorBrandForegroundOnLightSelected: LIGHT,
    colorBrandStroke1: bg,
    colorBrandStroke2: hover,
    colorCompoundBrandStroke: bg,
    colorCompoundBrandStrokeHover: hover,
    colorCompoundBrandStrokePressed: pressed,
    colorNeutralForeground2BrandHover: fg,
    colorNeutralForeground2BrandPressed: pressed,
    colorNeutralForeground2BrandSelected: fg,
    colorNeutralForeground3BrandHover: fg,
    colorNeutralForeground3BrandPressed: pressed,
    colorNeutralForeground3BrandSelected: fg,
    colorNeutralStrokeAccessibleSelected: bg,
    colorNeutralForegroundOnBrand: onBrand,
  };
}

export const nidongLightTheme = paint(webLightTheme, LIGHT, LIGHT_HOVER, LIGHT_PRESSED, LIGHT, "#FFFFFF");
export const nidongDarkTheme = paint(webDarkTheme, DARK, DARK_HOVER, DARK_PRESSED, DARK, "#0B1220");
