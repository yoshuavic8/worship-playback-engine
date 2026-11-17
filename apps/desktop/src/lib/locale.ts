export type UiLocale = "en" | "id";

let active: UiLocale = "en";

export function setUiLocale(locale: UiLocale) {
  active = locale === "id" ? "id" : "en";
  document.documentElement.lang = active;
}

export function getUiLocale(): UiLocale {
  return active;
}

/** BCP 47 tag for `Intl` / `toLocaleDateString`. English uses the browser default. */
export function localeTag(locale: UiLocale = active): string | undefined {
  return locale === "id" ? "id-ID" : undefined;
}

const PHRASES = {
  en: { sundayService: "Sunday Service" },
  id: { sundayService: "Ibadah Minggu" },
} as const;

export function t(key: keyof (typeof PHRASES)["en"], locale: UiLocale = active): string {
  return PHRASES[locale][key] ?? PHRASES.en[key];
}
