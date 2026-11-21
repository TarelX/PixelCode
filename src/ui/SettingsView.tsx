import { useCliDetection } from "../app/cli-detection";
import { useI18n, useLocale, type Locale } from "../i18n";
import { CliStatus } from "./components/CliStatus";
import { PixelButton } from "./components/PixelButton";

const languages: Locale[] = ["zh-CN", "en"];

export function SettingsView() {
  useCliDetection(true);
  const { locale, theme } = useI18n();
  const { change, saving, error } = useLocale();
  const language = theme.language;
  return <>
    <fieldset className="language-setting" aria-busy={saving}>
      <legend className="section-label">{language.title}</legend>
      <div className="language-setting__options">
        {languages.map((option) => <PixelButton
          key={option}
          lang={option}
          variant={locale === option ? "primary" : "secondary"}
          aria-pressed={locale === option}
          aria-disabled={saving}
          onClick={() => void change(option)}
        >{language[option]}</PixelButton>)}
      </div>
      {error && <p role="alert">{language.error}{error}</p>}
    </fieldset>
    <CliStatus />
  </>;
}
