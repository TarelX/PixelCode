import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useCliDetection } from "../app/cli-detection";
import { useI18n, useLocale, type Locale } from "../i18n";
import { CliStatus } from "./components/CliStatus";
import { PixelButton } from "./components/PixelButton";

const languages: Locale[] = ["zh-CN", "en"];

export function SettingsView() {
  useCliDetection(true);
  const { locale, theme } = useI18n();
  const { change, saving, error } = useLocale();
  const [notifications, setNotifications] = useState(false);
  const [loadingNotifications, setLoadingNotifications] = useState(true);
  const [notificationError, setNotificationError] = useState<string | null>(null);
  useEffect(() => {
    let cancelled = false;
    invoke<boolean>("get_notifications_enabled")
      .then((enabled) => { if (!cancelled) setNotifications(enabled); })
      .catch((reason) => { if (!cancelled) setNotificationError(String(reason)); })
      .finally(() => { if (!cancelled) setLoadingNotifications(false); });
    return () => { cancelled = true; };
  }, []);
  async function setNotificationEnabled(enabled: boolean) {
    const previous = notifications;
    setNotifications(enabled);
    setNotificationError(null);
    try {
      await invoke("set_notifications_enabled", { enabled });
    } catch (reason) {
      setNotifications(previous);
      setNotificationError(String(reason));
    }
  }
  const language = theme.language;
  const notice = theme.notifications;
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
    <fieldset className="language-setting" aria-busy={loadingNotifications}>
      <legend className="section-label">{notice.title}</legend>
      <p>{notice.hint}</p>
      <div className="language-setting__options">
        <PixelButton
          variant={notifications ? "primary" : "secondary"}
          aria-pressed={notifications}
          aria-disabled={loadingNotifications}
          onClick={() => void setNotificationEnabled(true)}
        >{notice.on}</PixelButton>
        <PixelButton
          variant={!notifications ? "primary" : "secondary"}
          aria-pressed={!notifications}
          aria-disabled={loadingNotifications}
          onClick={() => void setNotificationEnabled(false)}
        >{notice.off}</PixelButton>
      </div>
      {notificationError && <p role="alert">{notice.error}{notificationError}</p>}
    </fieldset>
    <CliStatus />
  </>;
}
